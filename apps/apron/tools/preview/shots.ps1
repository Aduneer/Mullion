<#
.SYNOPSIS
Renders the README's screenshots (docs/images) from the preview harness, on a made-up desktop.

.DESCRIPTION
Starts tools/preview/serve.py on a spare port, opens the harness in headless Edge, Chrome or
another Chromium browser, saves each picture, and stops the server.

With -Icons it instead renders assets/apron.svg and makes the app icons in src-tauri/icons
from it, and the tray icon from the same picture (needs Python with Pillow).

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
$userData = Join-Path $env:TEMP "apron-shots"

function Shot($url, $file, $width, $height, $background = "00000000", $budget = 6000) {
    Save-Shot $Browser $userData $url $file $width $height $background $budget
}

if ($Icons) {
    $logo = Join-Path $root "assets\apron.svg"
    New-AppIcons $Browser $userData $logo (Join-Path $root "src-tauri\icons") "apron" $logo
    return
}

$port = 5290
$server = Start-Process python -ArgumentList "-I", (Join-Path $PSScriptRoot "serve.py"), $port -PassThru -WindowStyle Hidden
try {
    Start-Sleep -Seconds 1
    $images = Join-Path $root "docs\images"
    New-Item -ItemType Directory -Force $images | Out-Null
    $base = "http://127.0.0.1:$port/tools/preview/?x=1"
    # The first run of a fresh browser profile can be cut short before the page loads: warm it up.
    Shot "$base&mode=dark" (Join-Path $env:TEMP "apron-warmup.png") 1600 900
    Shot "$base&mode=dark" (Join-Path $images "dock-dark.png") 1600 900
    Shot "$base&mode=light&glass=1&size=large&palette=catppuccin" (Join-Path $images "dock-light.png") 1600 900
    Shot "$base&mode=dark&float=1&palette=nord" (Join-Path $images "float.png") 1600 900
    Shot "$base&mode=dark&edge=left&size=small&palette=gruvbox" (Join-Path $images "side.png") 1600 900
    Shot "http://127.0.0.1:$port/ui/settings.html?mode=dark" (Join-Path $images "settings-dark.png") 640 760
    Shot "http://127.0.0.1:$port/ui/settings.html?mode=light&palette=catppuccin&float=1" (Join-Path $images "settings-light.png") 640 760

    # The repo's social preview (GitHub wants 1280x640): the name beside the desktop with the dock,
    # in the style of the other apps'. Upload it under the repo's Settings.
    $raw = Join-Path $env:TEMP "apron-social-desktop.png"
    Shot "$base&mode=dark&glass=1" $raw 1600 900
    New-SocialPreview $Browser $userData $raw (Join-Path $root "assets\apron.svg") "Apron" "A dock for Windows." `
        (Join-Path $images "social-preview.png") "440px" "96px" `
        "position:absolute;left:580px;top:80px;width:700px;height:560px;border-radius:16px 0 0 0" "position:absolute;left:-200px;bottom:0;width:1100px" "#4a5560"
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
