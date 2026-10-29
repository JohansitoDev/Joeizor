# Joeizor

**Creado por Johan Antinio Mancebo Lebron.** Joeizor es una base para crear
aplicaciones web con archivos `.je`, un compilador Rust que genera JavaScript
DOM directo y un servidor local con rutas por archivo.

## Requisitos

- Rust y Cargo (edition 2024).
- Un navegador moderno.
- Conexion a Internet para cargar Bootstrap 5.3.8 desde jsDelivr.

## Crear y ejecutar una app

```powershell
cargo run -- create mi-app
cd mi-app
cargo run --manifest-path ..\Cargo.toml -- run
```

El comando `run` levanta el servidor en `http://localhost:5173` y abre el
navegador. Al guardar archivos `.je`, el compilador los vuelve a procesar y el
navegador importa de nuevo el modulo de la ruta actual sin refrescar el
documento completo. El observador revisa cambios cada 25 ms. `run` tambien
intenta iniciar `jok run` si encuentra un `main.jok` con contenido; `dev` solo
ejecuta el frontend.

## Rutas y archivos

Las rutas web se derivan de los nombres de archivo:

| Archivo | Ruta |
| --- | --- |
| `views/home.je` | `/` |
| `views/dashboard.je` | `/dashboard` |
| `views/admin/users.je` | `/admin/users` |

Usa `navig to="/dashboard" ... end` para enlazar rutas. Cada destino debe tener
el archivo `.je` correspondiente. `public/index.html` es el documento anfitrion
y `public/` sirve imagenes y otros recursos estaticos.

## Comandos

Desde el repositorio de Joeizor:

```powershell
cargo run -- check views\home.je
cargo run -- build views\home.je dist\app.js
cargo run -- dev
cargo test
```

`jok-ui dev` sirve la carpeta actual en el puerto 5173. `jok-ui run` tambien
inicia el backend Jokotin cuando detecta codigo en `main.jok`.

## Lenguaje `.je`

```je
state count = 0
name = "Johan"
PI = 3.1416
users = ["Ana", "Luis"]

class Animal
  func speak()
    text "Sonido base" end
  end
end

class Cat extends Animal
  func speak()
    text class="text-primary" "Metodo sobrescrito" end
  end
end

cat = new Cat()
cat.speak()

sec class="container"
  for user in users
    text class="badge text-bg-light" "Hola {user}" end
  end
  if count
    text "Con valor" end
  else if name
    text "Hola {name}" end
  else
    text "Sin nombre" end
  end
  while count
    text "Mientras la condicion sea verdadera" end
  end
  do
    text "El bloque do se ejecuta al menos una vez" end
  end while count
end
```

`while` y `do ... end while condicion` compilan a bucles JavaScript. Las clases
usan `extends` y los metodos de JavaScript; sobrescribir un metodo en una clase
hija permite despacho polimorfico. Las variables llevan nombre en minusculas y
las constantes en mayusculas.

## Extension de VS Code e icono

La extension local esta en
[`extensions/joeizor-vscode`](./extensions/joeizor-vscode). Reconoce `.je`,
usa scopes TextMate compatibles con los colores Ruby del tema instalado, y
ofrece el tema `Joeizor File Icons` con el icono adjunto. Para desarrollo,
abre el proyecto en VS Code y ejecuta **Debug: Start Debugging** con una
configuracion de Extension Host. Para distribuir un `.vsix`, empaqueta esa
carpeta con `@vscode/vsce`; luego activa `Joeizor File Icons` como tema de
iconos de archivos.

## Ejemplo Bootstrap

`views/home.je` y `views/dashboard.je` forman el ejemplo de prueba. La pagina
incluye Bootstrap 5.3.8 por CDN, enlaces SPA, clases `container`, `card`,
`btn`, `alert` y `badge`, estructuras de control e herencia.

## Alcance actual

Esta version inicial compila `.je` a JavaScript y actualiza las rutas en el
navegador. El CSS de Bootstrap se carga por CDN; el compilador no descarga ni
purga Bootstrap o Tailwind. La recarga no conserva actualmente el estado de runtime. El runtime WASM y la
reactividad granular son trabajo pendiente, no funciones incluidas en este
prototipo.
