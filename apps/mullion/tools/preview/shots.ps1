<#
.SYNOPSIS
Renders the README's picture of Mullion Settings (docs/images) from the preview, on made-up data.

.DESCRIPTION
Starts tools/preview/serve.py on a spare port, opens the settings page in headless Edge, Chrome or
another Chromium browser, saves each picture, and stops the server.

With -Icons it instead renders assets/mullion.svg and makes the app and tray icons in
src-tauri/icons from it (needs Python with Pillow).

With -Installer it renders the pictures in Mullion's installer, the sidebar of its first and last
pages and the header of the others, into the repo's installer/ folder (24-bit BMPs, which is what
NSIS needs).

With -Social it renders the repo's social preview (docs/images/social-preview.png, 1280x640):
the name and line beside assets/social-window.svg, the logo's window at dusk. It names no app, so
it needn't change as apps are added. Upload it under the repo's Settings, General, Social preview.

.EXAMPLE
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1

.EXAMPLE
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1 -Installer
#>
param([switch]$Icons, [switch]$Installer, [switch]$Social, [string]$Browser)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
# The scripts every app shares (Shots.psm1, icons.py) are in the repo's own tools folder.
$shared = (Resolve-Path (Join-Path $root "..\..\tools")).Path

Import-Module (Join-Path $shared "preview\Shots.psm1") -Force

if (-not $Browser) { $Browser = Find-Browser }
$userData = Join-Path $env:TEMP "mullion-shots"

if ($Icons) {
    $logo = Join-Path $root "assets\mullion.svg"
    New-AppIcons $Browser $userData $logo (Join-Path $root "src-tauri\icons") "mullion" $logo
    return
}

if ($Social) {
    $file = Join-Path $root "docs\images\social-preview.png"
    New-SocialPreview $Browser $userData (Join-Path $root "assets\social-window.svg") (Join-Path $root "assets\mullion.svg") `
        "Mullion" "Well-made parts for the Windows desktop." $file "470px" "112px" `
        "position:absolute;left:560px;top:0;width:700px;height:640px;box-shadow:none!important" "width:700px;height:640px" `
        "#161513"
    Write-Host "  $file"
    return
}

if ($Installer) {
    Add-Type -AssemblyName System.Drawing
    $apps = (Resolve-Path (Join-Path $root "..")).Path
    $out = (Resolve-Path (Join-Path $root "..\..\installer")).Path
    $url = { param($path) ([Uri]$path).AbsoluteUri }
    $logo = & $url (Join-Path $root "assets\mullion.svg")
    $tiles = foreach ($a in "oriel", "lintel", "fanlight", "bay") { "<img src=`"$(& $url (Join-Path $apps "$a\assets\$a.svg"))`">" }
    $fonts = "'Segoe UI Variable Display','Segoe UI',sans-serif"
    # Drawn at four times the size and scaled down, so the edges stay smooth.
    $S = 4
    $pictures = [ordered]@{
        # The first and last pages' sidebar: the logo, the name and the line on the suite's dark,
        # with the apps' colours along the top and their logos along the bottom.
        "sidebar" = @{ w = 164; h = 314; css =
            "body{background:#161513;color:#f0efec;font-family:$fonts;text-align:center}" +
            "body:before{content:'';display:block;height:5px;background:linear-gradient(90deg,#2e6e60 25%,#b84d35 25% 50%,#dd9d2c 50% 75%,#2f5d8c 75%)}" +
            ".logo{display:block;width:64px;height:64px;margin:62px auto 16px}" +
            "h1{margin:0 0 9px;font-weight:600;font-size:32px;line-height:1;letter-spacing:-.6px}" +
            "p{margin:0 auto;width:124px;font-size:11.5px;line-height:1.4;color:#b8b5ad}" +
            ".apps{position:absolute;left:0;right:0;bottom:26px;display:flex;justify-content:center;gap:8px}" +
            ".apps img{width:24px;height:24px}"
            html = "<img class=`"logo`" src=`"$logo`"><h1>Mullion</h1><p>Well-made parts for the Windows desktop</p><div class=`"apps`">$($tiles -join '')</div>" }
        # The other pages' header, at the right of their white top band.
        "header" = @{ w = 150; h = 57; css =
            "body{background:#fff;display:flex;align-items:center;justify-content:flex-end;gap:9px;padding-right:14px;box-sizing:border-box;font-family:$fonts}" +
            "img{width:34px;height:34px}" +
            "b{font-weight:600;font-size:21px;letter-spacing:-.3px;color:#1c1b19}"
            html = "<img src=`"$logo`"><b>Mullion</b>" }
    }
    foreach ($name in $pictures.Keys) {
        $pic = $pictures[$name]
        $page = Join-Path $env:TEMP "mullion-installer-$name.html"
        Set-Content $page -Encoding utf8 ("<!doctype html><meta charset=`"utf-8`"><style>" +
            "html{zoom:$S}html,body{margin:0;width:$($pic.w)px;height:$($pic.h)px;overflow:hidden;position:relative}" +
            $pic.css + "</style>" + $pic.html)
        $png = Join-Path $env:TEMP "mullion-installer-$name.png"
        Save-Shot $Browser $userData (& $url $page) $png ($pic.w * $S) ($pic.h * $S) "ffffffff" 4000
        $src = [Drawing.Image]::FromFile($png)
        $bmp = New-Object Drawing.Bitmap($pic.w, $pic.h, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
        $g = [Drawing.Graphics]::FromImage($bmp)
        $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $g.DrawImage($src, (New-Object Drawing.Rectangle(0, 0, $pic.w, $pic.h)), 0, 0, $pic.w * $S, $pic.h * $S, [Drawing.GraphicsUnit]::Pixel)
        $g.Dispose(); $src.Dispose()
        $bmp.Save((Join-Path $out "$name.bmp"), [Drawing.Imaging.ImageFormat]::Bmp)
        $bmp.Dispose()
        Write-Host "  $(Join-Path $out "$name.bmp")"
    }
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
