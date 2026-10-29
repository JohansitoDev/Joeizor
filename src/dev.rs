use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use base64::Engine;
use sha1::{Digest, Sha1};

use crate::{compiler, parser};

const DEFAULT_DEV_PORT: u16 = 5173;
const DEV_PORT_RANGE: u16 = 100;

pub fn serve(root: PathBuf, run_backend: bool) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("no se pudo abrir {}: {error}", root.display()))?;
    build_wasm(&root)?;
    let listener = bind_dev_server()?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("no se pudo consultar el puerto del servidor: {error}"))?
        .port();
    let _backend = if run_backend {
        start_backend(&root)?
    } else {
        None
    };
    let generation = Arc::new(AtomicU64::new(0));
    watch_files(root.clone(), Arc::clone(&generation));

    let url = format!("http://localhost:{port}");
    println!("Joeizor disponible en {url}");
    open_browser(&url);
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

fn bind_dev_server() -> Result<TcpListener, String> {
    let last_port = DEFAULT_DEV_PORT.saturating_add(DEV_PORT_RANGE);
    if let Some(listener) = bind_available_port(DEFAULT_DEV_PORT, last_port)? {
        return Ok(listener);
    }

    TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("no se pudo iniciar el servidor en un puerto disponible: {error}"))
}

fn bind_available_port(first_port: u16, last_port: u16) -> Result<Option<TcpListener>, String> {
    for port in first_port..=last_port {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return Ok(Some(listener)),
            Err(error) if error.kind() == ErrorKind::AddrInUse => {}
            Err(error) => {
                return Err(format!(
                    "no se pudo iniciar el servidor en el puerto {port}: {error}"
                ));
            }
        }
    }
    Ok(None)
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
    if path == "/__joeizor/ws"
        && request
            .lines()
            .any(|line| line.eq_ignore_ascii_case("upgrade: websocket"))
    {
        return handle_websocket(stream, &request, generation);
    }
    if path == "/__joeizor/app.js" {
        let route = query_value(query, "route").unwrap_or_else(|| "/".to_owned());
        return match render_route(root, &route) {
            Ok(js) => respond(&mut stream, 200, "text/javascript; charset=utf-8", &js),
            Err((status, error)) => {
                respond(&mut stream, status, "text/plain; charset=utf-8", &error)
            }
        };
    }
    let decoded_path = percent_decode(path);
    let relative = if decoded_path == "/" || Path::new(&decoded_path).extension().is_none() {
        "index.html"
    } else {
        decoded_path.trim_start_matches('/')
    };
    serve_public_file(&mut stream, root, relative)
}

pub fn build_wasm(root: &Path) -> Result<(), String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let target_dir = root.join(".joeizor/target");
    let wasm = target_dir.join("wasm32-unknown-unknown/release/joeizor_ui.wasm");
    let output_dir = root.join("public/pkg");
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("no se pudo crear {}: {error}", output_dir.display()))?;

    let mut cargo = cargo_command();
    cargo
        .args([
            "build",
            "--release",
            "--lib",
            "--target",
            "wasm32-unknown-unknown",
            "--target-dir",
        ])
        .arg(&target_dir)
        .arg("--manifest-path")
        .arg(&manifest);
    run_command(
        &mut cargo,
        "compilacion WASM; instala el target con `rustup target add wasm32-unknown-unknown`",
    )?;
    run_command(
        wasm_bindgen_command()
            .arg(&wasm)
            .args(["--target", "web", "--out-dir"])
            .arg(&output_dir)
            .args(["--out-name", "joeizor_ui"]),
        "generacion del glue JS; instala wasm-bindgen-cli con `cargo install wasm-bindgen-cli`",
    )?;
    let wasm_size = fs::metadata(output_dir.join("joeizor_ui_bg.wasm"))
        .map_err(|error| format!("no se pudo medir el modulo WASM generado: {error}"))?
        .len();
    println!("Runtime WASM listo: {wasm_size} bytes");
    Ok(())
}

fn cargo_command() -> Command {
    let mut command = Command::new("cargo");
    if cfg!(windows) {
        let has_gnu_toolchain = Command::new("rustup")
            .args(["toolchain", "list"])
            .output()
            .ok()
            .is_some_and(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|line| line.starts_with("stable-x86_64-pc-windows-gnu"))
            });
        if has_gnu_toolchain {
            command.arg("+stable-x86_64-pc-windows-gnu");
        }
    }
    command
}

fn wasm_bindgen_command() -> Command {
    let executable = if cfg!(windows) {
        "wasm-bindgen.exe"
    } else {
        "wasm-bindgen"
    };
    let cargo_bin = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".cargo")))
        .map(|home| home.join("bin").join(executable));
    if let Some(path) = cargo_bin.filter(|path| path.is_file()) {
        Command::new(path)
    } else {
        Command::new("wasm-bindgen")
    }
}

fn run_command(command: &mut Command, failure: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("{failure}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{failure}.\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn handle_websocket(
    mut stream: TcpStream,
    request: &str,
    generation: &AtomicU64,
) -> Result<(), String> {
    let key = request
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("sec-websocket-key")
                .then(|| value.trim())
        })
        .ok_or("WebSocket sin Sec-WebSocket-Key")?;
    let accept = websocket_accept(key);
    write!(
        stream,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    )
    .map_err(|error| format!("no se pudo aceptar WebSocket: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(25)))
        .map_err(|error| format!("no se pudo configurar WebSocket: {error}"))?;

    let mut last_generation = generation.load(Ordering::Acquire);
    let mut frame = [0; 512];
    loop {
        match stream.read(&mut frame) {
            Ok(0) => return Ok(()),
            Ok(size) => {
                if size >= 2 {
                    let opcode = frame[0] & 0x0f;
                    if opcode == 0x8 {
                        send_frame(&mut stream, 0x8, &[])?;
                        return Ok(());
                    }
                    if opcode == 0x9 {
                        let masked = frame[1] & 0x80 != 0;
                        let mut offset = 2;
                        let payload_size = (frame[1] & 0x7f) as usize;
                        if payload_size == 126 {
                            if size < 4 {
                                continue;
                            }
                            offset = 4;
                        } else if payload_size == 127 {
                            return Err("WebSocket ping demasiado grande".to_owned());
                        }
                        let mask = if masked {
                            if size < offset + 4 {
                                continue;
                            }
                            let key = [
                                frame[offset],
                                frame[offset + 1],
                                frame[offset + 2],
                                frame[offset + 3],
                            ];
                            offset += 4;
                            Some(key)
                        } else {
                            None
                        };
                        if size < offset + payload_size {
                            continue;
                        }
                        let mut payload = frame[offset..offset + payload_size].to_vec();
                        if let Some(key) = mask {
                            for (index, byte) in payload.iter_mut().enumerate() {
                                *byte ^= key[index % key.len()];
                            }
                        }
                        send_frame(&mut stream, 0xA, &payload)?;
                    }
                }
            }
            Err(error)
                if error.kind() == ErrorKind::WouldBlock || error.kind() == ErrorKind::TimedOut => {
            }
            Err(error) => return Err(format!("error leyendo WebSocket: {error}")),
        }

        let current = generation.load(Ordering::Acquire);
        if current != last_generation {
            last_generation = current;
            send_frame(&mut stream, 0x1, b"reload")?;
        }
    }
}

fn websocket_accept(key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
}

fn send_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> Result<(), String> {
    if payload.len() > 125 {
        return Err("mensaje WebSocket supera el tamano permitido".to_owned());
    }
    stream
        .write_all(&[0x80 | opcode, payload.len() as u8])
        .and_then(|_| stream.write_all(payload))
        .map_err(|error| format!("no se pudo enviar WebSocket: {error}"))
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::{bind_available_port, websocket_accept};

    #[test]
    fn selects_the_next_available_port_when_a_port_is_occupied() {
        let occupied = TcpListener::bind(("127.0.0.1", 0)).expect("puerto efimero disponible");
        let occupied_port = occupied.local_addr().expect("puerto ocupado").port();
        let next_port = occupied_port.saturating_add(1);
        let Some(listener) =
            bind_available_port(occupied_port, next_port).expect("búsqueda de puerto disponible")
        else {
            assert_eq!(occupied_port, u16::MAX);
            return;
        };
        let selected = listener.local_addr().expect("puerto local").port();

        assert_eq!(selected, next_port);
        drop(occupied);
    }

    #[test]
    fn computes_rfc6455_accept_key() {
        assert_eq!(
            websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }
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
    let scope = if route.is_empty() {
        "/".to_owned()
    } else {
        format!("/{}", route.replace('\\', "/"))
    };
    Ok(compiler::compile_with_scope(&program, &scope))
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
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
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
    let mut previous = snapshot(&root);
    thread::spawn(move || {
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

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd").args(["/C", "start", "", url]).spawn();

    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(url).spawn();

    if let Err(error) = result {
        eprintln!("jok-ui: no se pudo abrir el navegador automaticamente: {error}");
    }
}
