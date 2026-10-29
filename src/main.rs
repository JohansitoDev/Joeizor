mod ast;
mod compiler;
mod dev;
mod parser;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("jok-ui: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_owned());
    match command.as_str() {
        "build" | "check" => {
            let input = PathBuf::from(args.next().unwrap_or_else(|| "views/home.je".to_owned()));
            let source = fs::read_to_string(&input)
                .map_err(|error| format!("no se pudo leer {}: {error}", input.display()))?;
            let program =
                parser::parse(&source).map_err(|error| format!("{}: {error}", input.display()))?;
            if command == "check" {
                println!("OK: {} se puede analizar.", input.display());
                return Ok(());
            }
            let output = PathBuf::from(args.next().unwrap_or_else(|| "dist/app.js".to_owned()));
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("no se pudo crear {}: {error}", parent.display()))?;
            }
            fs::write(&output, compiler::compile(&program))
                .map_err(|error| format!("no se pudo escribir {}: {error}", output.display()))?;
            println!("Compilado {} -> {}", input.display(), output.display());
            Ok(())
        }
        "create" => {
            let name = args.next().ok_or("uso: jok-ui create <nombre>")?;
            create_project(&name)
        }
        "dev" | "run" => {
            let root = PathBuf::from(args.next().unwrap_or_else(|| ".".to_owned()));
            dev::serve(root, command == "run")
        }
        _ => {
            println!(
                "Joeizor UI - compilador inicial de JOKOTIN UI\n\
                      Uso:\n\
                        jok-ui check [archivo.je]\n\
                        jok-ui build [archivo.je] [salida.js]\n\
                        jok-ui dev\n\
                        jok-ui create <nombre>"
            );
            Ok(())
        }
    }
}

fn create_project(name: &str) -> Result<(), String> {
    let root = Path::new(name);
    if root.exists() {
        return Err(format!(
            "la ruta {} ya existe; no se modifico",
            root.display()
        ));
    }
    fs::create_dir_all(root.join("views"))
        .map_err(|error| format!("no se pudo crear el proyecto: {error}"))?;
    fs::write(root.join("views/home.je"), include_str!("../views/home.je"))
        .map_err(|error| format!("no se pudo crear home.je: {error}"))?;
    fs::write(
        root.join("views/dashboard.je"),
        include_str!("../views/dashboard.je"),
    )
    .map_err(|error| format!("no se pudo crear dashboard.je: {error}"))?;
    fs::create_dir_all(root.join("public"))
        .map_err(|error| format!("no se pudo crear public: {error}"))?;
    fs::write(
        root.join("public/index.html"),
        include_str!("../public/index.html"),
    )
    .map_err(|error| format!("no se pudo crear index.html: {error}"))?;
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("public/joeizor.png"),
        root.join("public/joeizor.png"),
    )
    .map_err(|error| format!("no se pudo copiar el icono del proyecto: {error}"))?;
    let project_name = root
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or("Joeizor UI app");
    let config = format!(
        "{{\n  \"name\": \"{}\",\n  \"author\": \"Johan Antinio Mancebo Lebron\",\n  \"backend\": \"main.jok\",\n  \"entry\": \"views/home.je\",\n  \"description\": \"Creado por Johan Antinio Mancebo Lebron\"\n}}\n",
        json_escape(project_name)
    );
    fs::write(root.join("jok.json"), config)
        .map_err(|error| format!("no se pudo crear jok.json: {error}"))?;
    fs::write(
        root.join("main.jok"),
        "// Backend Jokotin: agrega aqui tus rutas API.\n",
    )
    .map_err(|error| format!("no se pudo crear main.jok: {error}"))?;
    println!("Proyecto creado en {}", root.display());
    Ok(())
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
