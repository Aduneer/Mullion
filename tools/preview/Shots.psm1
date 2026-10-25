# What every app's tools/preview/shots.ps1 shares: finding a browser, screenshotting a page in it
# headlessly, making the app icons from the logo, and the repo's social preview. Each app's script
# keeps its own list of pictures.

# Edge, Chrome or another Chromium browser, or throw if there's none.
function Find-Browser {
    $found = @(
        "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
        "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe",
        "$env:LOCALAPPDATA\imput\Helium\Application\chrome.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $found) { throw "No Edge or Chrome found; pass -Browser path\to\chrome.exe" }
    $found
}

# A screenshot of $Url at $Width x $Height. Taken with reduced motion, so whatever opens is fully
# open, unless -Moving.
function Save-Shot($Browser, $UserData, $Url, $File, $Width, $Height, $Background = "00000000", $Budget = 12000, [switch]$Moving) {
    Remove-Item $File -ErrorAction SilentlyContinue
    $motion = @()
    if (-not $Moving) { $motion += "--force-prefers-reduced-motion" }
    # The browser logs harmless noise to stderr, which Windows PowerShell would treat as an error.
    $ErrorActionPreference = "Continue"
    & $Browser --headless=new --disable-gpu --hide-scrollbars --no-first-run "--user-data-dir=$UserData" $motion `
        "--default-background-color=$Background" "--window-size=$Width,$Height" "--virtual-time-budget=$Budget" `
        "--screenshot=$File" $Url 2>&1 | Out-Null
    $ErrorActionPreference = "Stop"
    if (-not (Test-Path $File)) { throw "No screenshot for $Url" }
    Write-Host "  $File"
}

# The app icons in $Out (src-tauri/icons) from the logo, rendered at 1024 px (needs Python with
# Pillow, for tools/icons.py). With $Tray, also tray.png from that picture.
function New-AppIcons($Browser, $UserData, $Svg, $Out, $Name, $Tray) {
    $png = Join-Path $env:TEMP "$Name-logo-1024.png"
    Save-Shot $Browser $UserData ([Uri]$Svg).AbsoluteUri $png 1024 1024
    $extra = @()
    if ($Tray) {
        $extra = Join-Path $env:TEMP "$Name-tray-1024.png"
        Save-Shot $Browser $UserData ([Uri]$Tray).AbsoluteUri $extra 1024 1024
    }
    python -I (Join-Path $PSScriptRoot "..\icons.py") $png $Out $extra
}

# The repo's social preview (GitHub wants 1280x640): the logo, name and tagline on the left, and
# $Shot (a screenshot of the app) framed on the right. $TextWidth, $TitleSize, $Frame and $Picture
# are CSS for the text column's width, the name's size, the frame and the screenshot in it, and
# $Accent the app's own color, along the top.
function New-SocialPreview($Browser, $UserData, $Shot, $Logo, $Name, $Tagline, $File, $TextWidth, $TitleSize, $Frame, $Picture, $Accent = "#f0efec") {
    $page = Join-Path $env:TEMP "$($Name.ToLower())-social.html"
    $url = { param($path) ([Uri]$path).AbsoluteUri }
    Set-Content $page -Encoding utf8 ("<!doctype html><meta charset=`"utf-8`"><style>" +
        "html,body{margin:0;width:1280px;height:640px;overflow:hidden}" +
        "body{background:#161513;border-top:8px solid $Accent;box-sizing:border-box;" +
        "color:#f0efec;font-family:'Segoe UI Variable Text','Segoe UI',sans-serif}" +
        ".t{position:absolute;left:80px;top:0;bottom:0;width:$TextWidth;display:flex;flex-direction:column;justify-content:center}" +
        ".t img{width:104px;height:104px;margin-bottom:28px}" +
        "h1{margin:0 0 18px;font:600 $TitleSize/1 'Segoe UI Variable Display','Segoe UI',sans-serif;letter-spacing:-2px}" +
        "p{margin:0;font-size:33px;line-height:1.3;color:#b8b5ad}" +
        ".s{margin-top:30px;font-size:21px;color:#898781}" +
        ".d{$Frame;overflow:hidden;box-shadow:0 30px 90px #000a,0 0 0 1px #ffffff14}" +
        ".d img{$Picture}" +
        "</style><div class=`"d`"><img src=`"$(& $url $Shot)`"></div>" +
        "<div class=`"t`"><img src=`"$(& $url $Logo)`"><h1>$Name</h1>" +
        "<p>$Tagline</p><div class=`"s`">Free and open source &middot; Rust + Tauri</div></div>")
    Save-Shot $Browser $UserData (& $url $page) $File 1280 640 "161513ff"
}

Export-ModuleMember -Function Find-Browser, Save-Shot, New-AppIcons, New-SocialPreview
