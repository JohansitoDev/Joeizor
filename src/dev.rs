use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::{compiler, parser};

pub fn serve(root: PathBuf, run_backend: bool) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("no se pudo abrir {}: {error}", root.display()))?;
    let listener = TcpListener::bind(("127.0.0.1", 5173))
        .map_err(|error| format!("no se pudo iniciar http://localhost:5173: {error}"))?;
    let _backend = if run_backend {
        start_backend(&root)?
    } else {
        None
    };
    let generation = Arc::new(AtomicU64::new(0));
    watch_files(root.clone(), Arc::clone(&generation));

    println!("Joeizor disponible en http://localhost:5173");
    open_browser();
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let root = root.clone();
                let generation = Arc::clone(&generation);
                thread::spawn(move || {
                    if let Err(error) = handle_request(stream, &root, &generation) {
                        eprintln!("jok-ui: error HTTP: {error}");
                    }
                });
            }
            Err(error) => eprintln!("jok-ui: conexion rechazada: {error}"),
        }
    }
    Ok(())
}

fn start_backend(root: &Path) -> Result<Option<Child>, String> {
    let entry = root.join("main.jok");
    if !entry.exists() {
        return Ok(None);
    }
    let source = fs::read_to_string(&entry)
        .map_err(|error| format!("no se pudo leer {}: {error}", entry.display()))?;
    if !source
        .lines()
        .any(|line| !line.trim().is_empty() && !line.trim().starts_with("//"))
    {
        return Ok(None);
    }

    println!("Iniciando backend con `jok run`...");
    let child = Command::new("jok")
        .arg("run")
        .current_dir(root)
        .stdin(Stdio::null())
        .spawn()
        .map_err(|error| format!("no se pudo iniciar `jok run` para main.jok: {error}"))?;
    Ok(Some(child))
}

fn handle_request(
    mut stream: TcpStream,
    root: &Path,
    generation: &AtomicU64,
) -> Result<(), String> {
    let mut buffer = [0; 8192];
    let size = stream
        .read(&mut buffer)
        .map_err(|error| format!("no se pudo leer la solicitud: {error}"))?;
    let request = String::from_utf8_lossy(&buffer[..size]);
    let first_line = request.lines().next().unwrap_or_default();
    let mut fields = first_line.split_whitespace();
    let method = fields.next().unwrap_or_default();
    let target = fields.next().unwrap_or("/");
    if method != "GET" {
        return respond(&mut stream, 405, "text/plain", "Method Not Allowed");
    }

    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path == "/__joeizor/app.js" {
        let route = query_value(query, "route").unwrap_or("/");
        return match render_route(root, route) {
            Ok(js) => respond(&mut stream, 200, "text/javascript; charset=utf-8", &js),
            Err((status, error)) => {
                respond(&mut stream, status, "text/plain; charset=utf-8", &error)
            }
        };
    }
    if path == "/__joeizor/events" {
        let previous = generation.load(Ordering::Acquire);
        for _ in 0..250 {
            thread::sleep(Duration::from_millis(100));
            let current = generation.load(Ordering::Acquire);
            if current != previous {
                return respond(
                    &mut stream,
                    200,
                    "text/event-stream",
                    &format!("data: {current}\n\n"),
                );
            }
        }
        return respond(&mut stream, 200, "text/event-stream", ": keep-alive\n\n");
    }

    let decoded_path = percent_decode(path);
    let relative = if decoded_path == "/" || Path::new(&decoded_path).extension().is_none() {
        "index.html"
    } else {
        decoded_path.trim_start_matches('/')
    };
    serve_public_file(&mut stream, root, relative)
}

fn render_route(root: &Path, route: &str) -> Result<String, (u16, String)> {
    let route = route.trim_start_matches('/');
    let relative = if route.is_empty() {
        PathBuf::from("home.je")
    } else {
        PathBuf::from(route)
    };
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err((400, "Ruta de pagina no valida.".to_owned()));
    }
    let views = root
        .join("views")
        .canonicalize()
        .map_err(|error| (404, format!("No se encontro views: {error}")))?;
    let unresolved = views.join(relative).with_extension("je");
    let file = unresolved
        .canonicalize()
        .map_err(|error| (404, format!("No se encontro la pagina: {error}")))?;
    if !file.starts_with(&views) {
        return Err((400, "Ruta de pagina fuera de views.".to_owned()));
    }
    let source = fs::read_to_string(&file).map_err(|error| {
        (
            404,
            format!("No se encontro la pagina {}: {error}", file.display()),
        )
    })?;
    let program = parser::parse(&source)
        .map_err(|error| (422, format!("Error en {}: {error}", file.display())))?;
    Ok(compiler::compile(&program))
}

fn serve_public_file(stream: &mut TcpStream, root: &Path, relative: &str) -> Result<(), String> {
    let public = root.join("public");
    let Ok(canonical_public) = public.canonicalize() else {
        return serve_embedded_file(stream, relative);
    };
    let candidate = public.join(relative);
    let Ok(file) = candidate.canonicalize() else {
        return serve_embedded_file(stream, relative);
    };
    if !file.starts_with(&canonical_public) || !file.is_file() {
        return respond(stream, 404, "text/plain", "Not Found");
    }
    let content =
        fs::read(&file).map_err(|error| format!("no se pudo leer el archivo: {error}"))?;
    let content_type = match file.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    };
    respond_bytes(stream, 200, content_type, &content)
}

fn serve_embedded_file(stream: &mut TcpStream, relative: &str) -> Result<(), String> {
    match relative {
        "index.html" => respond(
            stream,
            200,
            "text/html; charset=utf-8",
            include_str!("../public/index.html"),
        ),
        "joeizor.png" => respond_bytes(
            stream,
            200,
            "image/png",
            include_bytes!("../public/joeizor.png"),
        ),
        _ => respond(stream, 404, "text/plain", "Not Found"),
    }
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    respond_bytes(stream, status, content_type, body.as_bytes())
}

fn respond_bytes(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let label = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        422 => "Unprocessable Entity",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {label}\r\nContent-Type: {content_type}\r\n\
         Content-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\r\n",
        body.len()
    )
    .and_then(|_| stream.write_all(body))
    .map_err(|error| format!("no se pudo enviar respuesta: {error}"))
}

fn query_value(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key == name).then(|| percent_decode(value))
    })
}

fn percent_decode(value: &str) -> String {
    let mut decoded = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    decoded.push(byte);
                    index += 3;
                    continue;
                }
            }
        }
        decoded.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn watch_files(root: PathBuf, generation: Arc<AtomicU64>) {
    thread::spawn(move || {
        let mut previous = snapshot(&root);
        loop {
            thread::sleep(Duration::from_millis(25));
            let current = snapshot(&root);
            if current != previous {
                previous = current;
                generation.fetch_add(1, Ordering::Release);
            }
        }
    });
}

fn snapshot(root: &Path) -> HashMap<PathBuf, (u64, Option<SystemTime>)> {
    let mut entries = HashMap::new();
    collect_files(&root.join("views"), &mut entries);
    entries
}

fn collect_files(directory: &Path, entries: &mut HashMap<PathBuf, (u64, Option<SystemTime>)>) {
    let Ok(items) = fs::read_dir(directory) else {
        return;
    };
    for item in items.flatten() {
        let path = item.path();
        let Ok(file_type) = item.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_files(&path, entries);
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "je") {
            if let Ok(metadata) = path.metadata() {
                entries.insert(path, (metadata.len(), metadata.modified().ok()));
            }
        }
    }
}

fn open_browser() {
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd")
        .args(["/C", "start", "", "http://localhost:5173"])
        .spawn();

    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg("http://localhost:5173").spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let result = Command::new("xdg-open")
        .arg("http://localhost:5173")
        .spawn();

    if let Err(error) = result {
        eprintln!("jok-ui: no se pudo abrir el navegador automaticamente: {error}");
    }
}
