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
} finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
