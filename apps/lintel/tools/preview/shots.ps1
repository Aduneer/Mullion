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

function Shot($url, $file, $width, $height, $background = "00000000", $budget = 12000, [switch]$Moving) {
    Remove-Item $file -ErrorAction SilentlyContinue
    $motion = @()
    if (-not $Moving) { $motion += "--force-prefers-reduced-motion" }
    # The browser logs harmless noise to stderr, which Windows PowerShell would treat as an error.
    $ErrorActionPreference = "Continue"
    & $Browser --headless=new --disable-gpu --hide-scrollbars --no-first-run "--user-data-dir=$userData" $motion `
        "--default-background-color=$background" "--window-size=$width,$height" "--virtual-time-budget=$budget" `
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
    # The first run of a fresh browser profile can be cut short before the menu opens: warm it up.
    Shot "$base&theme=dark&open=media" (Join-Path $env:TEMP "lintel-warmup.png") 1440 620
    if ($Demo) {
        # 20 frames a second of one page load, its clock stepped frame by frame (demo.py), then an
        # animated WebP like Oriel's: smaller than a GIF, and not cut down to 256 colors.
        $ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
        if (-not $ffmpeg) { throw "The demo needs ffmpeg on the PATH: winget install Gyan.FFmpeg" }
        $frames = Join-Path $env:TEMP "lintel-demo"
        Remove-Item $frames -Recurse -Force -ErrorAction SilentlyContinue
        python -I (Join-Path $PSScriptRoot "demo.py") $Browser "$base&theme=dark&demo=1" $frames 700 10300 20
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
    $page = Join-Path $env:TEMP "lintel-social.html"
    $file = { param($path) ([Uri]$path).AbsoluteUri }
    Set-Content $page -Encoding utf8 ("<!doctype html><meta charset=`"utf-8`"><style>" +
        "html,body{margin:0;width:1280px;height:640px;overflow:hidden}" +
        "body{background:radial-gradient(circle at 88% 10%,#9085e938,transparent 50%),radial-gradient(circle at 0 100%,#3987e52e,transparent 45%),linear-gradient(#1a1a21,#0c0c10);" +
        "color:#f0efec;font-family:'Segoe UI Variable Text','Segoe UI',sans-serif}" +
        ".t{position:absolute;left:80px;top:0;bottom:0;width:440px;display:flex;flex-direction:column;justify-content:center}" +
        ".t img{width:104px;height:104px;margin-bottom:28px}" +
        "h1{margin:0 0 18px;font:600 100px/1 'Segoe UI Variable Display','Segoe UI',sans-serif;letter-spacing:-2px}" +
        "p{margin:0;font-size:33px;line-height:1.3;color:#b4b8c2}" +
        ".s{margin-top:30px;font-size:21px;color:#7c818d}" +
        ".d{position:absolute;left:610px;top:110px;width:640px;height:560px;border-radius:16px;overflow:hidden;box-shadow:0 30px 90px #000a,0 0 0 1px #ffffff14}" +
        ".d img{position:absolute;right:0;top:0;width:1440px}" +
        "</style><div class=`"d`"><img src=`"$(& $file $raw)`"></div>" +
        "<div class=`"t`"><img src=`"$(& $file (Join-Path $root 'assets\lintel.svg'))`"><h1>Lintel</h1>" +
        "<p>A menu bar for Windows.</p><div class=`"s`">Free and open source &middot; Rust + Tauri</div></div>")
    Shot (& $file $page) (Join-Path $images "social-preview.png") 1280 640 "1a1a21ff"
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
