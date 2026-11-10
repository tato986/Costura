param(
    [string]$ProcessName = "costura",
    [Parameter(Mandatory = $true)][int]$X,
    [Parameter(Mandatory = $true)][int]$Y,
    [switch]$DoubleClick
)

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class WinClick {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } |
    Select-Object -First 1
if (-not $proc) { throw "No hay ventana abierta para '$ProcessName'." }

$handle = $proc.MainWindowHandle
[WinClick]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 250

$rect = New-Object WinClick+RECT
[WinClick]::GetWindowRect($handle, [ref]$rect) | Out-Null
$screenX = $rect.Left + $X
$screenY = $rect.Top + $Y

[WinClick]::SetCursorPos($screenX, $screenY) | Out-Null
Start-Sleep -Milliseconds 120
[WinClick]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
[WinClick]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
if ($DoubleClick) {
    Start-Sleep -Milliseconds 80
    [WinClick]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    [WinClick]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
}
Write-Output "Clic en ($X,$Y) -> ($screenX,$screenY)"
