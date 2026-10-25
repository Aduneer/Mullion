<#
.SYNOPSIS
Renders the README's screenshots (docs/images) from the preview harness, on made-up data.

.DESCRIPTION
Starts tools/preview/serve.py on a spare port, opens the harness in headless Edge, Chrome or
another Chromium browser, saves each picture, and stops the server.

With -Demo it instead renders the README's animation (docs/images/demo.webp) from ?demo=1 with
demo.py; that needs ffmpeg.

With -Icons it instead renders assets/fanlight.svg and makes the app icons in src-tauri/icons
from it (needs Python with Pillow).

.EXAMPLE
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1

.EXAMPLE
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1 -Demo
#>
param([switch]$Icons, [switch]$Demo, [string]$Browser)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
# The scripts every app shares (Shots.psm1, icons.py, demo.py) are in the repo's own tools folder.
$shared = (Resolve-Path (Join-Path $root "..\..\tools")).Path

Import-Module (Join-Path $shared "preview\Shots.psm1") -Force

if (-not $Browser) { $Browser = Find-Browser }
$userData = Join-Path $env:TEMP "fanlight-shots"

function Shot($url, $file, $width, $height, $background = "00000000", $budget = 6000) {
    Save-Shot $Browser $userData $url $file $width $height $background $budget
}

if ($Icons) {
    New-AppIcons $Browser $userData (Join-Path $root "assets\fanlight.svg") (Join-Path $root "src-tauri\icons") "fanlight"
    return
}

$port = 5296
$server = Start-Process python -ArgumentList "-I", (Join-Path $PSScriptRoot "serve.py"), $port -PassThru -WindowStyle Hidden
try {
    Start-Sleep -Seconds 1
    $images = Join-Path $root "docs\images"
    New-Item -ItemType Directory -Force $images | Out-Null
    $base = "http://127.0.0.1:$port/tools/preview/?x=1"
    # The first run of a fresh browser profile can be cut short before the panel opens: warm it up.
    Shot "$base&q=code" (Join-Path $env:TEMP "fanlight-warmup.png") 1280 640
    if ($Demo) {
        # 20 frames a second of one page load, recorded as it plays (demo.py), then an animated
        # WebP like Oriel's and Lintel's: smaller than a GIF, and not cut down to 256 colors.
        $ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
        if (-not $ffmpeg) { throw "The demo needs ffmpeg on the PATH: winget install Gyan.FFmpeg" }
        $frames = Join-Path $env:TEMP "fanlight-demo"
        Remove-Item $frames -Recurse -Force -ErrorAction SilentlyContinue
        python -I (Join-Path $shared "preview\demo.py") $Browser "$base&demo=1" $frames 300 15800 20 1280 800
        $webp = Join-Path $images "demo.webp"
        & $ffmpeg -hide_banner -loglevel error -y -framerate 20 -i (Join-Path $frames "%03d.png") `
            -vf "scale=1200:-1:flags=lanczos" -c:v libwebp_anim -quality 80 -loop 0 $webp
        Get-Item $webp | Format-Table FullName, @{ n = "KB"; e = { [math]::Round($_.Length / 1KB) } } -AutoSize
        return
    }
    $lisbon = [Uri]::EscapeDataString("lisbon")
    Shot "$base&q=code" (Join-Path $images "search-dark.png") 1280 640
    Shot "$base&q=code&theme=light" (Join-Path $images "search-light.png") 1280 640
    Shot "$base&q=$lisbon&sel=1&preview=1" (Join-Path $images "preview.png") 1280 800
    Shot "$base&q=$lisbon&sel=3&preview=1&theme=light&palette=nord" (Join-Path $images "preview-text.png") 1280 800
    Shot "$base&q=$([Uri]::EscapeDataString('10 km in mi'))&palette=gruvbox" (Join-Path $images "calc.png") 1280 440
    Shot "$base&only=clipboard&sel=2&preview=1&glass=on&palette=catppuccin" (Join-Path $images "clipboard.png") 1280 800
    Shot "$base&only=actions&palette=rose-pine" (Join-Path $images "actions.png") 1280 800
    Shot "$base&only=apps&palette=nord" (Join-Path $images "launchpad.png") 1280 800
    Shot "$base&q=$([Uri]::EscapeDataString('(1200+340)*1.2'))&glass=on" (Join-Path $images "glass.png") 1280 440
    Shot "$base&view=settings" (Join-Path $images "settings.png") 1280 800

    # The repo's social preview (GitHub wants 1280x640): the name beside the panel with the
    # preview open, in the style of Oriel's and Lintel's. Upload it under the repo's Settings.
    $raw = Join-Path $env:TEMP "fanlight-social-panel.png"
    Shot "$base&q=$lisbon&sel=1&preview=1&glass=on" $raw 1280 800
    New-SocialPreview $Browser $userData $raw (Join-Path $root "assets\fanlight.svg") "Fanlight" "Search and launch for Windows." `
        (Join-Path $images "social-preview.png") "460px" "96px" `
        "position:absolute;left:600px;top:96px;width:680px;height:560px;border-radius:16px 0 0 0" "position:absolute;left:-74px;top:-100px;width:1280px" "#dd9d2c"
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
