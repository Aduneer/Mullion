<#
.SYNOPSIS
Renders the README's screenshots (docs/images) from the preview harness, on made-up data.

.DESCRIPTION
Starts tools/preview/serve.py on a spare port, opens the harness in headless Edge, Chrome or
another Chromium browser with a fixed clock, saves each picture, and stops the server.

Stills are taken with reduced motion, so each menu is fully open. With -Demo it instead renders
the README's animation (docs/images/demo.webp) from ?demo=1 with demo.py; that needs ffmpeg.

With -Icons it instead renders assets/lintel.svg and makes the app icons in src-tauri/icons from
it (needs Python with Pillow).

.EXAMPLE
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1

.EXAMPLE
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1 -Icons
#>
param([switch]$Icons, [switch]$Demo, [string]$Browser)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
# The scripts every app shares (Shots.psm1, icons.py, demo.py) are in the repo's own tools folder.
$shared = (Resolve-Path (Join-Path $root "..\..\tools")).Path

Import-Module (Join-Path $shared "preview\Shots.psm1") -Force

if (-not $Browser) { $Browser = Find-Browser }
$userData = Join-Path $env:TEMP "lintel-shots"

function Shot($url, $file, $width, $height, $background = "00000000", $budget = 12000, [switch]$Moving) {
    Save-Shot $Browser $userData $url $file $width $height $background $budget -Moving:$Moving
}

if ($Icons) {
    New-AppIcons $Browser $userData (Join-Path $root "assets\lintel.svg") (Join-Path $root "src-tauri\icons") "lintel"
    return
}

$port = 5298
$server = Start-Process python -ArgumentList "-I", (Join-Path $PSScriptRoot "serve.py"), $port -PassThru -WindowStyle Hidden
try {
    Start-Sleep -Seconds 1
    $images = Join-Path $root "docs\images"
    New-Item -ItemType Directory -Force $images | Out-Null
    $base = "http://127.0.0.1:$port/tools/preview/?time=2026-10-24T17:42:00"
    # The first run of a fresh browser profile can be cut short before the menu opens: warm it up.
    Shot "$base&theme=dark&open=media" (Join-Path $env:TEMP "lintel-warmup.png") 1440 620
    if ($Demo) {
        # 20 frames a second of one page load, recorded as it plays (demo.py), then an animated
        # WebP like Oriel's: smaller than a GIF, and not cut down to 256 colors. The page is 1440x449,
        # what the browser's window left it before demo.py set the size itself.
        $ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
        if (-not $ffmpeg) { throw "The demo needs ffmpeg on the PATH: winget install Gyan.FFmpeg" }
        $frames = Join-Path $env:TEMP "lintel-demo"
        Remove-Item $frames -Recurse -Force -ErrorAction SilentlyContinue
        python -I (Join-Path $shared "preview\demo.py") $Browser "$base&theme=dark&demo=1" $frames 700 10300 20 1440 449
        $webp = Join-Path $images "demo.webp"
        & $ffmpeg -hide_banner -loglevel error -y -framerate 20 -i (Join-Path $frames "%03d.png") `
            -vf "scale=1280:-1:flags=lanczos" -c:v libwebp_anim -quality 82 -loop 0 $webp
        Get-Item $webp | Format-Table FullName, @{ n = "KB"; e = { [math]::Round($_.Length / 1KB) } } -AutoSize
        return
    }
    Shot "$base&theme=dark&open=media" (Join-Path $images "bar-dark.png") 1440 620
    Shot "$base&theme=light&open=media" (Join-Path $images "bar-light.png") 1440 620
    Shot "$base&theme=dark&open=app" (Join-Path $images "app-menu.png") 1440 420
    Shot "$base&theme=dark&open=stats&palette=catppuccin" (Join-Path $images "stats.png") 1440 520
    Shot "$base&theme=dark&edge=left&open=clock&palette=nord" (Join-Path $images "side.png") 900 620
    Shot "$base&theme=dark&open=control" (Join-Path $images "control.png") 1440 560
    Shot "$base&theme=dark&glass=on&open=clock" (Join-Path $images "glass.png") 1440 520

    # The repo's social preview (GitHub wants 1280x640): the name beside a glass bar with the
    # Control Centre open, in the style of Oriel's. Upload it under the repo's Settings.
    $raw = Join-Path $env:TEMP "lintel-social-bar.png"
    Shot "$base&theme=dark&glass=on&open=control" $raw 1440 640
    New-SocialPreview $Browser $userData $raw (Join-Path $root "assets\lintel.svg") "Lintel" "A menu bar for Windows." `
        (Join-Path $images "social-preview.png") "440px" "100px" `
        "position:absolute;left:610px;top:110px;width:640px;height:560px;border-radius:16px" "position:absolute;right:0;top:0;width:1440px" "#b84d35"
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
