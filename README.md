# Joeizor

**Creado por Johan Antinio Mancebo Lebron.**

Joeizor es, a la vez, un **lenguaje de programación**, un **lenguaje de marcado
de interfaces** y un **framework frontend**. Se escriben las vistas en `.je`,
con sintaxis propia tipo Ruby, y el compilador/CLI está escrito en Rust. No
necesitas escribir etiquetas HTML en las vistas Joeizor; `public/index.html`
es solamente el documento anfitrión que carga la aplicación en el navegador.

## Requisitos

- Rust estable y Cargo (edition 2024).
- Target `wasm32-unknown-unknown`.
- `wasm-bindgen-cli` versión `0.2.129` para generar los archivos del runtime.
- Navegador moderno.
- Conexión a Internet para cargar Bootstrap 5.3.8 desde jsDelivr en el ejemplo.

En Windows, los comandos de abajo usan el toolchain GNU, que evita depender de
Visual C++ Build Tools. Si tienes ese toolchain, prepara WASM así:

```powershell
rustup target add --toolchain stable-x86_64-pc-windows-gnu wasm32-unknown-unknown
cargo +stable-x86_64-pc-windows-gnu install wasm-bindgen-cli --version 0.2.129
```

La instalación de `wasm-bindgen-cli` en Windows también requiere herramientas
de enlace compatibles. Si falla por falta de `dlltool`, instala GNU Binutils o
descarga el binario oficial `wasm-bindgen` versión `0.2.129` y colócalo en
`%USERPROFILE%\.cargo\bin`.

## Crear y ejecutar una aplicación

Desde la raíz de este repositorio, en PowerShell:

```powershell
cargo +stable-x86_64-pc-windows-gnu run -- create mi-app
Set-Location mi-app
cargo +stable-x86_64-pc-windows-gnu run --manifest-path ..\Cargo.toml -- run
```

El comando `run` construye el runtime WASM, inicia el servidor frontend e
imprime la URL en la terminal antes de abrirla en el navegador (normalmente
`http://localhost:5173`). Si ese puerto está ocupado, prueba automáticamente
los siguientes 100 y, si hace falta, solicita un puerto libre al sistema. Si
`main.jok` contiene código, también intenta iniciar el backend con `jok run`.
Para levantar solo el frontend, usa `dev` en lugar de `run`:

```powershell
cargo +stable-x86_64-pc-windows-gnu run --manifest-path ..\Cargo.toml -- dev
```

El servidor empieza a observar los archivos `.je` de `views/` al arrancar.
Detecta cambios con un sondeo de 25 ms y envía invalidaciones por WebSocket,
también cuando se usa un puerto alternativo. El navegador vuelve a importar la
vista activa sin recargar el documento y conserva los valores declarados con
`state`. El tiempo total de recompilación depende del tamaño del proyecto y
del equipo.

### Instalar el CLI desde este repositorio

También puedes instalar el ejecutable `jok-ui` en Cargo:

```powershell
cargo +stable-x86_64-pc-windows-gnu install --path . --locked
jok-ui create mi-app
Set-Location mi-app
jok-ui run
```

Para comprobar o compilar vistas del proyecto actual:

```powershell
jok-ui check views\home.je
jok-ui build views\home.je dist\app.js
jok-ui wasm-build
jok-ui dev
jok-ui run
```

Si Visual C++ Build Tools está instalado, puedes omitir
`+stable-x86_64-pc-windows-gnu` de los comandos `cargo`. En macOS y Linux usa
`cargo` directamente.

## Estructura de una aplicación

`jok-ui create mi-app` crea una estructura inicial como esta:

```text
mi-app/
├── main.jok
├── jok.json
├── public/
│   ├── index.html
│   └── joeizor.png
└── views/
    ├── home.je
    └── dashboard.je
```

Las rutas web se derivan de los nombres de archivo:

| Archivo | Ruta |
| --- | --- |
| `views/home.je` | `/` |
| `views/dashboard.je` | `/dashboard` |
| `views/admin/users.je` | `/admin/users` |

Usa `navig to="/dashboard" ... end` para enlazar páginas. Cada destino debe
tener su archivo `.je` en `views/`. El compilador genera
`public/pkg/joeizor_ui.js` y `public/pkg/joeizor_ui_bg.wasm` al ejecutar `run`,
`dev` o `wasm-build`.

## Sintaxis `.je`

```je
state count = 0
name = "Johan"
PI = 3.1416
users = ["Ana", "Luis", "Marta"]

func Greeting()
  text class="display-5 fw-bold text-primary" "Hola, {name}" end
end

sec class="container py-5"
  Greeting()
  text class="lead" "Contador: {count}" end

  btn class="btn btn-success" onClick={count++}
    text "Sumar" end
  end

  navig to="/dashboard" class="btn btn-primary"
    text "Abrir dashboard" end
  end

  for user in users
    text class="badge text-bg-light" "Hola {user}" end
  end

  if count
    text "El contador tiene valor." end
  else if name
    text "Hola {name}" end
  else
    text "Sin nombre" end
  end
end
```

`state` crea una señal alojada en WASM. Las interpolaciones actualizan el nodo
de texto afectado y las condiciones reactivas actualizan su propio fragmento.
También se admiten variables, constantes, funciones/componentes, clases con
`extends`, métodos sobrescritos, `while` y `do ... end while`.

## GitHub y estadísticas de lenguaje

El repositorio incluye `.gitattributes` para clasificar los archivos `.je` como
`Joeizor` cuando GitHub Linguist reconozca el lenguaje. La barra **Languages**
de GitHub calcula el porcentaje según los bytes de código de cada lenguaje;
no es una métrica de instalaciones ni de uso de Joeizor en todo GitHub.

**Importante:** Joeizor todavía no está registrado en el catálogo de GitHub
Linguist. GitHub no incluye en las estadísticas un lenguaje que no aparezca en
ese catálogo, así que `.gitattributes` por sí solo no puede hacer aparecer
ahora mismo el nombre o su porcentaje. Para habilitarlo en GitHub hay que
proponer la extensión `.je` como nuevo lenguaje en
[github-linguist/linguist](https://github.com/github-linguist/linguist) y
esperar a que acepten y publiquen ese cambio. La configuración local ya queda
preparada para cuando Linguist lo incorpore.

## Extensión de VS Code e icono

La extensión local está en
[`extensions/joeizor-vscode`](./extensions/joeizor-vscode). Reconoce `.je`,
usa scopes TextMate compatibles con los colores Ruby del tema instalado y
ofrece el tema `Joeizor File Icons` con el icono del proyecto. Instálala con:

```powershell
code --install-extension .\extensions\joeizor-vscode\joeizor-language-0.1.0.vsix
```

Los colores de `.je` se detectan automáticamente; para el icono, selecciona
`Joeizor File Icons` en **Preferences: File Icon Theme**. La extensión todavía
no está publicada en Visual Studio Marketplace. Publicarla requiere una cuenta
Publisher y un token de acceso de Marketplace.

## Ejemplo Bootstrap y alcance actual

`views/home.je` y `views/dashboard.je` son ejemplos funcionales con clases
Bootstrap 5.3.8, enlaces SPA, estado reactivo, eventos, bucles y condiciones.
Bootstrap se carga desde CDN. Aunque puedes escribir clases Bootstrap en
`class=""`, el compilador todavía no descarga ni purga Bootstrap o Tailwind y
no incluye un bundler completo.

El runtime WASM utiliza `wasm-bindgen`/`web-sys` para señales y operaciones DOM;
el compilador Rust genera los módulos JavaScript de las vistas. HMR conserva
las señales de `state`, pero no las instancias DOM ni estado local fuera de
`state`. Este proyecto es una base funcional en desarrollo, no una
implementación completa de todas las características de un lenguaje general.
