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
$userData = Join-Path $env:TEMP "fanlight-shots"

function Shot($url, $file, $width, $height, $background = "00000000", $budget = 6000) {
    Remove-Item $file -ErrorAction SilentlyContinue
    # The browser logs harmless noise to stderr, which Windows PowerShell would treat as an error.
    $ErrorActionPreference = "Continue"
    & $Browser --headless=new --disable-gpu --hide-scrollbars --no-first-run "--user-data-dir=$userData" --force-prefers-reduced-motion `
        "--default-background-color=$background" "--window-size=$width,$height" "--virtual-time-budget=$budget" `
        "--screenshot=$file" $url 2>&1 | Out-Null
    $ErrorActionPreference = "Stop"
    if (-not (Test-Path $file)) { throw "No screenshot for $url" }
    Write-Host "  $file"
}

if ($Icons) {
    $png = Join-Path $env:TEMP "fanlight-logo-1024.png"
    Shot ([Uri](Join-Path $root "assets\fanlight.svg")).AbsoluteUri $png 1024 1024
    python -I (Join-Path $root "tools\icons.py") $png (Join-Path $root "src-tauri\icons")
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
        python -I (Join-Path $PSScriptRoot "demo.py") $Browser "$base&demo=1" $frames 300 15800 20 1280 800
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
    $page = Join-Path $env:TEMP "fanlight-social.html"
    $file = { param($path) ([Uri]$path).AbsoluteUri }
    Set-Content $page -Encoding utf8 ("<!doctype html><meta charset=`"utf-8`"><style>" +
        "html,body{margin:0;width:1280px;height:640px;overflow:hidden}" +
        "body{background:radial-gradient(circle at 88% 10%,#9085e938,transparent 50%),radial-gradient(circle at 0 100%,#3987e52e,transparent 45%),linear-gradient(#1a1a21,#0c0c10);" +
        "color:#f0efec;font-family:'Segoe UI Variable Text','Segoe UI',sans-serif}" +
        ".t{position:absolute;left:80px;top:0;bottom:0;width:460px;display:flex;flex-direction:column;justify-content:center}" +
        ".t img{width:104px;height:104px;margin-bottom:28px}" +
        "h1{margin:0 0 18px;font:600 96px/1 'Segoe UI Variable Display','Segoe UI',sans-serif;letter-spacing:-2px}" +
        "p{margin:0;font-size:33px;line-height:1.3;color:#b4b8c2}" +
        ".s{margin-top:30px;font-size:21px;color:#7c818d}" +
        ".d{position:absolute;left:600px;top:96px;width:680px;height:560px;border-radius:16px 0 0 0;overflow:hidden;box-shadow:0 30px 90px #000a,0 0 0 1px #ffffff14}" +
        ".d img{position:absolute;left:-74px;top:-100px;width:1280px}" +
        "</style><div class=`"d`"><img src=`"$(& $file $raw)`"></div>" +
        "<div class=`"t`"><img src=`"$(& $file (Join-Path $root 'assets\fanlight.svg'))`"><h1>Fanlight</h1>" +
        "<p>Search and launch for Windows.</p><div class=`"s`">Free and open source &middot; Rust + Tauri</div></div>")
    Shot (& $file $page) (Join-Path $images "social-preview.png") 1280 640 "1a1a21ff"
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
