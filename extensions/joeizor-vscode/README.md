# Joeizor Language for VS Code

Extensión local para archivos `.je`, con resaltado basado en los scopes de color
Ruby habituales de TextMate y el icono de Joeizor para todos los archivos `.je`.

Los archivos `.je` se detectan automáticamente y reciben resaltado del tema
Ruby-compatible activo. Para instalar localmente, desde esta carpeta ejecuta:

```powershell
npm exec --yes --package=@vscode/vsce -- vsce package
code --install-extension .\joeizor-language-0.1.0.vsix
```

Selecciona **Joeizor File Icons** en `Preferences: File Icon Theme`; VS Code no
permite a una extensión cambiar automáticamente el tema de iconos del usuario.

La extensión aún no está publicada en Visual Studio Marketplace. Para publicarla
se necesita una cuenta Publisher verificada y un Personal Access Token propio;
el identificador previsto es `joeizor.joeizor-language`.
