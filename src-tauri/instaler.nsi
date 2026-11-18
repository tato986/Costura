; ==========================================================
; Script de Instalación NSIS para Costura Taller (Tauri 2.x)
; ==========================================================

!define APP_NAME "Costura Taller"
!define APP_VERSION "0.1.0"
!define APP_PUBLISHER "Lisandro Ramos"
!define APP_EXE "Costura.exe"
!define INSTALL_DIR_NAME "CosturaTaller"

; Configuración del Compresor (LZMA para menor tamaño)
SetCompressor /SOLID lzma

; Incluir Modern UI 2
!include "MUI2.nsh"
!include "LogicLib.nsh"

; ----------------------------------------------------------
; Atributos Generales del Instalador
Name "${APP_NAME}"
OutFile "CosturaTaller_Installer_${APP_VERSION}.exe"
InstallDir "$PROGRAMFILES64\${INSTALL_DIR_NAME}"
RequestExecutionLevel admin

; ----------------------------------------------------------
; Páginas del Instalador
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; Páginas del Desinstalador
!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

; Idioma
!insertmacro MUI_LANGUAGE "Spanish"

; ----------------------------------------------------------
; Sección Principal de Instalación
Section "CoreInstallation" SecCore
  SetOutPath "$INSTDIR"

  ; Copilar/Incluir el ejecutable desde la ruta de release de Tauri
  ; (Asegúrate de que Costura.exe esté en esta ruta relativa o cámbiala según necesites)
  File "..\src-tauri\target\release\Costura.exe"

  ; Crear el Desinstalador oficial en el directorio de la app
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Crear Accesos Directos (Escritorio y Menú Inicio)
  CreateShortCut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortCut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0

  ; Registrar la aplicación en el Panel de Control de Windows para una desinstalación limpia
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${INSTALL_DIR_NAME}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${INSTALL_DIR_NAME}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${INSTALL_DIR_NAME}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${INSTALL_DIR_NAME}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${INSTALL_DIR_NAME}" "QuietUninstallString" "$INSTDIR\uninstall.exe /S"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${INSTALL_DIR_NAME}" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${INSTALL_DIR_NAME}" "NoRepair" 1
SectionEnd

; ----------------------------------------------------------
; Sección de Desinstalación
Section "Uninstall"
  ; Eliminar archivos principales
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\uninstall.exe"

  ; Eliminar accesos directos
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  Delete "$DESKTOP\${APP_NAME}.lnk"

  ; Intentar limpiar la carpeta de instalación (solo si está vacía para no borrar datos locales de usuario en %APPDATA%)
  RMDir "$INSTDIR"

  ; Limpiar claves del registro de Windows
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${INSTALL_DIR_NAME}"
SectionEnd