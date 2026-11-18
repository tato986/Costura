# Estado de Preparación para Microsoft Store (Proyecto Completado)

Este documento resume el estado actual del proyecto **Taller Costura** tras completar la internacionalización, compilación de producción y empaquetado MSIX.

## 1. Cambios y Funcionalidades Implementadas

### A. Compatibilidad con Microsoft Store (MSIX) y Almacenamiento
1. **`src-tauri/src/store.rs`**: Añadido `dirs::data_local_dir()` (`%LOCALAPPDATA%\CosturaTaller`) como ruta principal antes de `current_exe()` para garantizar permisos de escritura dentro del contenedor MSIX de Windows.
2. **`src-tauri/src/main.rs`**: Registrados los comandos `commands::set_backup_dir` y `commands::choose_folder` en `tauri::generate_handler!`.
3. **`src-tauri/tauri.conf.json`**: Configurada la política de seguridad de contenido (CSP) de producción y las opciones de `bundle.windows` (`webviewInstallMode` y `nsis.installMode: "currentUser"`).

### B. Gestión Personalizada de Respaldos
1. **`src/js/app.js`**: Añadidos botones en **Ajustes → Dónde se guardan tus datos** para cambiar la carpeta de respaldos (`choose-backup-dir`) o restablecer la predeterminada (`reset-backup-dir`).

### C. Soporte Multi-idioma (Español, Inglés y Portugués)
1. **`src/js/i18n.js`**: Sistema de internacionalización completo para **Español (`es`)**, **Inglés (`en`)** y **Portugués (`pt`)**, incluyendo textos de interfaz, estados, prioridades, etapas de bitácora, categorías/unidades de insumos, alertas, modales, ficha de impresión y formatos de fecha/moneda locales.
2. **`src/js/garments.js` y `src/index.html`**: Traducción dinámica del catálogo de prendas, estilos, medidas, opciones y etiquetas de los patrones SVG.
3. **`src-tauri/src/model.rs` y `src-tauri/src/commands.rs`**: Añadido el campo `language` en `Settings` (por defecto `"es"`) y actualizado `save_settings` para persistir el idioma elegido en el archivo de datos local.
4. **Selector en Ajustes**: El usuario puede cambiar entre **Español**, **English** y **Português** en tiempo real desde **Ajustes → Datos del taller**.

### D. Manifiesto MSIX y Scripts de Compilación
1. **`src-tauri/msix/AppxManifest.xml`**:
   - Identidad configurada con `Name="JLRDev.TallerCostura"` y `Publisher="CN=1D206152-7DEC-4E64-832B-010FB525F11D"`.
   - Declarados los tres idiomas soportados en `<Resources>` para que Microsoft Partner Center los detecte automáticamente desde un único paquete:
     - `es-ES` (Español)
     - `en-US` (Inglés)
     - `pt-BR` (Portugués)
2. **`tools/build-store.ps1` y `compilar.bat`**:
   - Configurados para incluir automáticamente `%USERPROFILE%\.cargo\bin` en el `PATH`.
   - Compilan el binario optimizado (`cargo build --release`), actualizan `Costura.exe` en la raíz y empaquetan el `.msix` con `makeappx.exe` del Windows SDK.

---

## 2. Archivos Generados y Listos para Usar

- **Ejecutable portátil / directo (actualizado)**:
  `G:\costura\Costuraprogram\Costura.exe`
- **Binario Release de Cargo**:
  `G:\costura\Costuraprogram\src-tauri\target\release\costura.exe`
- **Paquete MSIX para subir a Microsoft Partner Center**:
  `G:\costura\Costuraprogram\src-tauri\target\release\TallerCostura_x64.msix`
- **Carpeta de staging del paquete MSIX**:
  `G:\costura\Costuraprogram\src-tauri\target\msix-stage\`

---

## 3. Próximos Pasos para Publicar en Microsoft Store

1. Entrar a **Microsoft Partner Center** y abrir la aplicación **Taller Costura**.
2. Crear un nuevo envío y subir el archivo:
   `src-tauri\target\release\TallerCostura_x64.msix`
3. Partner Center detectará automáticamente los idiomas **Español**, **Inglés** y **Portugués** gracias al `AppxManifest.xml`, permitiendo completar la descripción y capturas de pantalla en la tienda para los 3 idiomas.
4. Si en el futuro haces más cambios en el código, solo debes ejecutar:
   ```powershell
   powershell -ExecutionPolicy Bypass -File "tools\build-store.ps1"
   ```
