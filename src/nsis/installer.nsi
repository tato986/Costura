; NSIS installer for Costura - Gestión de taller de costura
; Versión 0.1.0
; Página para elegir la carpeta de respaldos

; -- Configuración general --
Name "Costura - Gestión de taller"
DisplayName "Costura - Gestión de taller"
DisplayVersion "0.1.0"
UninstallDisplayName "Costura - Gestión de taller"
Publisher "Taller de costura"
; -- Ruta de instalación --
InstallDir "$PROGRAMFILES64\\Costura"
; -- Idioma español --
!insertmacro INSTLNG_SUPPORTed
!define MUI_LANGUAGE "Spanish"
; -- Incluir las macros de la interfaz --
!include "MUI.nsh"
!include "WordFunc.nsh"
!include "FileFunc.nsh"
!registry.nsh
; -- Página de bienvenida --
Page directory
Page instfiles
; -- Página para elegir carpeta de respaldos --
!macro ChooseBackupDir
    ; Mostrar cuadro de diálogo para elegir carpeta
    RequestFolder "Elige la carpeta donde se guardarán los respaldos de Costura" "$INSTDIR\\backups"
    SetOutDir $INSTDIR
    ; Guardar la ruta elegida en el registro y en un archivo de configuración
    WriteRegStr HKLM "Software\\Costura\\Config" "BackupDir" "$DirResult"
    ; Crecer la carpeta si no existe
    ReadRegStr $0 HKLM "Software\\Costura\\Config" "BackupDir"
    StrLen $0 $0
    StrCmp $0 "" already_made_dir make_dir
    make_dir:
    SetOutDir $INSTDIR
    mkdir "$DirResult\\backups"
    already_made_dir:
!macroend
; -- Página de confirmación --
Page instfiles
; -- Página final --
Page instfiles
; -- Mensajes --
!define MUI_WELCOMEFINISHPAGE_RUN "$INSTDIR\\Costura.exe"
!define MUI_UNWELCOMEPAGE_DESC "Desinstala Costura - Gestión de taller"
; -- Paquete --
; Configurar el directorio de instalación
OutFile "Costura-Installer.exe"
; -- Configuración del instalador --
SetCompressor /ULTRA
SetDatablockOptimization on
SetInstallOptions --nocompression
; -- Secuencia de instalación --
Section "Secuencia principal" Sec0
    ; Ejecutar la elección de carpeta de respaldos antes de la instalación normal
    !insertmacro ChooseBackupDir
    
    ; Instalar los archivos
    SetOutDir $INSTDIR
    ; Copiar el ejecutable
    File "..\\..\\target\\release\\Costura.exe"
    ; Copiar los datos y configuración
    File "..\\..\\src-tauri\\costura.json"
    ; Crear carpetas necesarias
    mkdir "$INSTDIR\\datos"
    mkdir "$INSTDIR\\backups"
    mkdir "$INSTDIR\\webview"
    
    ; Crear acceso directo
    CreateDirectory "$SMPROGRAMS\\Costura"
    CreateShortcut "$SMPROGRAMS\\Costura\\Costura.lnk" "$INSTDIR\\Costura.exe"
    
    ; Escribir archivo de configuración inicial
    WriteINIStr "$INSTDIR\\costura.ini" "Config" "BackupDir" "$DirResult"
    WriteINIStr "$INSTDIR\\costura.ini" "Config" "Currency" "$"
    WriteINIStr "$INSTDIR\\costura.ini" "Config" "WorkshopName" "Mi taller de costura"
    
SectionEnd
; -- Secuencia de desinstalación --
Section -Sec0
    ; Remover acceso directo
    Delete "$SMPROGRAMS\\Costura\\Costura.lnk"
    RMDir "$SMPROGRAMS\\Costura"
    
    ; Remover archivos
    Delete "$INSTDIR\\Costura.exe"
    Delete "$INSTDIR\\costura.json"
    Delete "$INSTDIR\\datos"
    Delete "$INSTDIR\\backups"
    Delete "$INSTDIR\\webview"
    Delete "$INSTDIR\\costura.ini"
    
    ; Remover registro
    DeleteRegKey HKLM "Software\\Costura\\Config"
    
   ; Mostrar mensaje
    Message "Costura ha sido desinstalado correctamente."
SectionEnd