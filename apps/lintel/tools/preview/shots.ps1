<#
.SYNOPSIS
Renders the README's screenshots (docs/images) from the preview harness, on made-up data.

.DESCRIPTION
Starts tools/preview/serve.py on a spare port, opens the harness in headless Edge, Chrome or
another Chromium browser with a fixed clock, saves each picture, and stops the server.

With -Icons it instead renders assets/lintel.svg and makes the app icons in src-tauri/icons from
it (needs Python with Pillow).

.EXAMPLE
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1

.EXAMPLE
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1 -Icons
#>
param([switch]$Icons, [string]$Browser)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

if (-not $Browser) {
    $Browser = @(
        "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
        "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe",
        "$env:LOCALAPPDATA\imput\Helium\Application\chrome.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $Browser) { throw "No Edge or Chrome found; pass -Browser path\to\chrome.exe" }
}
$userData = Join-Path $env:TEMP "lintel-shots"

function Shot($url, $file, $width, $height, $background = "00000000") {
    Remove-Item $file -ErrorAction SilentlyContinue
    # The browser logs harmless noise to stderr, which Windows PowerShell would treat as an error.
    $ErrorActionPreference = "Continue"
    & $Browser --headless=new --disable-gpu --hide-scrollbars --no-first-run "--user-data-dir=$userData" `
        "--default-background-color=$background" "--window-size=$width,$height" --virtual-time-budget=6000 `
        "--screenshot=$file" $url 2>&1 | Out-Null
    $ErrorActionPreference = "Stop"
    if (-not (Test-Path $file)) { throw "No screenshot for $url" }
    Write-Host "  $file"
}

if ($Icons) {
    $png = Join-Path $env:TEMP "lintel-logo-1024.png"
    Shot ([Uri](Join-Path $root "assets\lintel.svg")).AbsoluteUri $png 1024 1024
    python -I (Join-Path $root "tools\icons.py") $png (Join-Path $root "src-tauri\icons")
    return
}

$port = 5298
$server = Start-Process python -ArgumentList "-I", (Join-Path $PSScriptRoot "serve.py"), $port -PassThru -WindowStyle Hidden
try {
    Start-Sleep -Seconds 1
    $images = Join-Path $root "docs\images"
    New-Item -ItemType Directory -Force $images | Out-Null
    $base = "http://127.0.0.1:$port/tools/preview/?time=2026-10-24T17:42:00"
    Shot "$base&theme=dark&open=media" (Join-Path $images "bar-dark.png") 1440 620
    Shot "$base&theme=light&open=media" (Join-Path $images "bar-light.png") 1440 620
    Shot "$base&theme=dark&open=app" (Join-Path $images "app-menu.png") 1440 420
    Shot "$base&theme=dark&open=stats&palette=catppuccin" (Join-Path $images "stats.png") 1440 520
    Shot "$base&theme=dark&edge=left&open=clock&palette=nord" (Join-Path $images "side.png") 900 620
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
