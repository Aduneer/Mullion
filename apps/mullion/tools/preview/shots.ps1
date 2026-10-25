<#
.SYNOPSIS
Renders the README's picture of Mullion Settings (docs/images) from the preview, on made-up data.

.DESCRIPTION
Starts tools/preview/serve.py on a spare port, opens the settings page in headless Edge, Chrome or
another Chromium browser, saves each picture, and stops the server.

With -Icons it instead renders assets/mullion.svg and assets/tray.svg and makes the app and tray
icons in src-tauri/icons from them (needs Python with Pillow).

.EXAMPLE
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1
#>
param([switch]$Icons, [string]$Browser)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
# The scripts every app shares (Shots.psm1, icons.py) are in the repo's own tools folder.
$shared = (Resolve-Path (Join-Path $root "..\..\tools")).Path

Import-Module (Join-Path $shared "preview\Shots.psm1") -Force

if (-not $Browser) { $Browser = Find-Browser }
$userData = Join-Path $env:TEMP "mullion-shots"

if ($Icons) {
    New-AppIcons $Browser $userData (Join-Path $root "assets\mullion.svg") (Join-Path $root "src-tauri\icons") "mullion" (Join-Path $root "assets\tray.svg")
    return
}

$port = 5294
$server = Start-Process python -ArgumentList "-I", (Join-Path $PSScriptRoot "serve.py"), $port -PassThru -WindowStyle Hidden
try {
    Start-Sleep -Seconds 1
    $images = Join-Path $root "docs\images"
    New-Item -ItemType Directory -Force $images | Out-Null
    $base = "http://127.0.0.1:$port/ui/index.html?x=1"
    Save-Shot $Browser $userData "$base&mode=dark" (Join-Path $env:TEMP "mullion-warmup.png") 560 680
    Save-Shot $Browser $userData "$base&mode=dark" (Join-Path $images "settings-dark.png") 560 680 "0d0d0dff" 4000
    Save-Shot $Browser $userData "$base&mode=light" (Join-Path $images "settings-light.png") 560 680 "f9f9f7ff" 4000
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
