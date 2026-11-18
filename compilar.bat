@echo off
setlocal
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
echo ===================================================
echo   Compilando Costura (Release + Actualizacion EXE)
echo ===================================================
cd /d "%~dp0src-tauri"
cargo build --release --offline
if errorlevel 1 (
    echo Reintentando sin modo offline...
    cargo build --release
    if errorlevel 1 (
        echo Error al compilar Costura.
        pause
        exit /b 1
    )
)
echo.
echo Copiando nuevo ejecutable a Costura.exe...
copy /Y "target\release\costura.exe" "%~dp0Costura.exe"
echo.
echo Listo: Costura.exe ha sido actualizado con la nueva version.
endlocal
