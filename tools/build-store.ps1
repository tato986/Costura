# Script para compilar Costura, actualizar Costura.exe y generar el paquete MSIX para Microsoft Store
$ErrorActionPreference = "Stop"

if ($env:PATH -notlike "*\.cargo\bin*") {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
}

$RootDir = Split-Path -Parent $PSScriptRoot
$TauriDir = Join-Path $RootDir "src-tauri"
$ReleaseDir = Join-Path $TauriDir "target\release"
$StageDir = Join-Path $TauriDir "target\msix-stage"

Write-Host "1. Compilando binario de producción (Release)..." -ForegroundColor Cyan
Push-Location $TauriDir
try {
    cargo build --release --offline
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Reintentando compilación sin --offline..." -ForegroundColor Yellow
        cargo build --release
        if ($LASTEXITCODE -ne 0) { throw "Falló la compilación de Cargo." }
    }
} finally {
    Pop-Location
}

$ExeSource = Join-Path $ReleaseDir "costura.exe"
$ExeRoot = Join-Path $RootDir "Costura.exe"

Write-Host "2. Actualizando Costura.exe en la raíz del proyecto..." -ForegroundColor Cyan
Copy-Item -Path $ExeSource -Destination $ExeRoot -Force
Write-Host "   -> Actualizado: $ExeRoot" -ForegroundColor Green

Write-Host "3. Preparando directorio de staging para MSIX..." -ForegroundColor Cyan
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force -Path (Join-Path $StageDir "Assets") | Out-Null

Copy-Item -Path $ExeSource -Destination (Join-Path $StageDir "costura.exe") -Force
Copy-Item -Path (Join-Path $TauriDir "msix\AppxManifest.xml") -Destination (Join-Path $StageDir "AppxManifest.xml") -Force

$IconSource = Join-Path $TauriDir "icons\icon.png"
if (Test-Path $IconSource) {
    Copy-Item -Path $IconSource -Destination (Join-Path $StageDir "Assets\StoreLogo.png") -Force
    Copy-Item -Path $IconSource -Destination (Join-Path $StageDir "Assets\Square44x44Logo.png") -Force
    Copy-Item -Path $IconSource -Destination (Join-Path $StageDir "Assets\Square150x150Logo.png") -Force
}

$MakeAppx = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
if ($MakeAppx) {
    $MsixOut = Join-Path $ReleaseDir "TallerCostura_x64.msix"
    Write-Host "4. Empaquetando MSIX con $($MakeAppx.FullName)..." -ForegroundColor Cyan
    & $MakeAppx.FullName pack /d $StageDir /p $MsixOut /o
    Write-Host "   -> Paquete MSIX listo para Microsoft Store: $MsixOut" -ForegroundColor Green
} else {
    Write-Host "4. Aviso: No se encontró makeappx.exe del Windows SDK. La carpeta lista para empaquetar está en: $StageDir" -ForegroundColor Yellow
}

Write-Host "¡Proceso completado con éxito!" -ForegroundColor Green
