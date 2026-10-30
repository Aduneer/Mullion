<#
.SYNOPSIS
Renders the README's screenshots (docs/images) from the preview harness, on made-up windows.

.DESCRIPTION
Starts tools/preview/serve.py on a spare port, opens the harness in headless Edge, Chrome or
another Chromium browser, saves each picture, and stops the server.

With -Icons it instead renders assets/sash.svg and makes the app icons in src-tauri/icons
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
$userData = Join-Path $env:TEMP "sash-shots"

function Shot($url, $file, $width, $height, $background = "00000000", $budget = 6000) {
    Save-Shot $Browser $userData $url $file $width $height $background $budget
}

if ($Icons) {
    $logo = Join-Path $root "assets\sash.svg"
    New-AppIcons $Browser $userData $logo (Join-Path $root "src-tauri\icons") "sash" $logo
    return
}

$port = 5293
$server = Start-Process python -ArgumentList "-I", (Join-Path $PSScriptRoot "serve.py"), $port -PassThru -WindowStyle Hidden
try {
    Start-Sleep -Seconds 1
    $images = Join-Path $root "docs\images"
    New-Item -ItemType Directory -Force $images | Out-Null
    $base = "http://127.0.0.1:$port/tools/preview/?x=1"
    # The first run of a fresh browser profile can be cut short before the page loads: warm it up.
    Shot $base (Join-Path $env:TEMP "sash-warmup.png") 1600 948
    Shot "$base&keys=ctrl%2Balt%2Bleft" (Join-Path $images "snapped-dark.png") 1600 948
    Shot "$base&keys=ctrl%2Balt%2Bleft&mode=light" (Join-Path $images "snapped-light.png") 1600 948
    Shot "$base&scene=thirds&gap=8" (Join-Path $images "thirds.png") 1600 948
    Shot "$base&scene=free&note=1" (Join-Path $images "note.png") 1600 948
    Shot "http://127.0.0.1:$port/ui/settings.html?mode=dark" (Join-Path $images "settings-dark.png") 600 760
    Shot "http://127.0.0.1:$port/ui/settings.html?mode=light&palette=catppuccin" (Join-Path $images "settings-light.png") 600 760

    # The repo's social preview (GitHub wants 1280x640): the name beside a snapped desktop, in the
    # style of the other apps'. Upload it under the repo's Settings.
    $raw = Join-Path $env:TEMP "sash-social-desktop.png"
    Shot "$base" $raw 1600 948
    New-SocialPreview $Browser $userData $raw (Join-Path $root "assets\sash.svg") "Sash" "Window snapping for Windows." `
        (Join-Path $images "social-preview.png") "440px" "96px" `
        "position:absolute;left:580px;top:96px;width:700px;height:560px;border-radius:16px 0 0 0" "position:absolute;left:0;top:0;width:960px" "#6b4c8a"
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
