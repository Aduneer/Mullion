<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/banner-dark.svg">
  <img src="assets/banner-light.svg" width="640" alt="Lintel">
</picture>

**A menu bar for Windows.**

The app you're in, what's playing, CPU and memory, volume, network, battery and the clock,<br>
in one slim bar along the top of your screen, the way macOS and Linux desktops have it.

[![MIT license](https://img.shields.io/badge/license-MIT-3987e5)](LICENSE)
![Windows 10 and 11](https://img.shields.io/badge/Windows-10%20%C2%B7%2011-3987e5)
![Built with Rust and Tauri 2](https://img.shields.io/badge/Rust-Tauri%202-9085e9?logo=rust)

[Install](#install) · [What's in it](#whats-in-the-bar) · [Positions and themes](#positions-and-themes) · [FAQ](#faq) · [Contributing](CONTRIBUTING.md)

</div>

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/images/bar-light.png">
  <img alt="Lintel along the top of the screen: the logo, Google Chrome as the app in front, then the track that's playing, CPU and memory, volume, Wi-Fi, battery and the clock, with the Now playing menu open under the track" src="docs/images/bar-dark.png">
</picture>

On a Mac or a Linux desktop, the top of the screen tells you which app you're in and gives you
its menu, with the clock, sound, network and battery in the corner. Windows puts all of that in
the taskbar, mixed in with the app buttons, or nowhere. Lintel is that bar for Windows: thin,
always there, and every item opens a small menu that does something.

## Why Lintel

- **It takes its own space.** Lintel registers with Windows as an app bar, like the taskbar
  does, so maximized windows stop below it instead of sliding underneath. Your taskbar stays
  where it is.
- **Menus, not settings pages.** Click an item for a menu right under it: skip a track, change
  an app's volume, switch to another window of the app in front, see the month. Click it again
  to close it, or slide along the bar to the next one.
- **Out of the way.** It hides while a game or video is full screen, and it never takes the
  keyboard away from the window you're working in.
- **Private.** No account, no telemetry, and it never goes online. See [SECURITY.md](SECURITY.md).
- **Looks like it belongs.** Six palettes, each in dark and light, and Auto to follow Windows.
  The same ones as [Oriel](https://github.com/Aduneer/Oriel).

## What's in the bar

From left to right:

- **The logo menu**: About This PC, System Settings, Task Manager, Sleep, Restart, Shut Down,
  Sign Out and Lock Screen, plus **Menu Bar Settings** and **Quit Lintel**. Restart, Shut Down
  and Sign Out ask first.
- **The app in front**, by name (*File Explorer*, *Terminal*, *Google Chrome*). Its menu lists
  that app's windows to switch to, and has Minimize, Close Window and Quit.
- **Now playing**: the track from Spotify, your browser or anything else that shows in Windows'
  media controls. The menu has the cover, play/pause, skip and how far in you are. Middle-click
  the item to play or pause.
- **CPU and memory**, with a minute of history for CPU, memory and network in the menu, and a
  shortcut to Task Manager.
- **Volume**: your output's volume and every app's, each with mute. Scroll over the item to
  change the volume without opening anything.
- **Network**: Wi-Fi, wired or offline, and whether you're on the internet.
- **Battery**, on laptops: charge, whether it's plugged in and the time left.
- **The clock**, 12- or 24-hour as Windows has it or as you pick, with the date and seconds if
  you want them. Its menu is a calendar that starts the week on your day.

<img alt="The menu of the app in front: Google Chrome's three windows, the minimized one marked, then Minimize, Close Window and Quit Google Chrome" src="docs/images/app-menu.png">

<img alt="The CPU and memory menu in the Catppuccin palette: a minute of history for CPU, memory, download and upload" src="docs/images/stats.png">

## Positions and themes

Lintel can sit along the **top**, **bottom**, **left** or **right** of your main screen
(Menu Bar Settings → Position). Down the side, the app's name runs vertically and the clock
stacks hours over minutes. Three sizes: Compact, Standard and Large.

<img alt="Lintel down the left side of the screen in the Nord palette, with the calendar open beside the clock" src="docs/images/side.png">

The palettes are Oriel, Catppuccin, Nord, Gruvbox, Rosé Pine and Solarized, each in dark and
light.

## Install

Lintel runs on Windows 10 and 11. There's no release yet, so for now
[build it from source](CONTRIBUTING.md#building-from-source): it takes Rust and a few minutes.

Once there are releases, they'll come as **Lintel-*version*-setup.exe** (installs just for you,
no admin prompt) and **Lintel-*version*-portable.zip**, like Oriel's.

> [!IMPORTANT]
> Lintel isn't code-signed yet, so Windows doesn't know who made it:
> - **SmartScreen** may say *Windows protected your PC*. Click **More info**, then **Run anyway**.
> - **Smart App Control**, if it's on, may block it, and it has no "run anyway". Building it
>   yourself works. Please don't turn Smart App Control off for Lintel: on most versions of
>   Windows it can only be turned back on by reinstalling Windows.

To uninstall an installed copy, go to **Settings → Apps → Installed apps → Lintel**. Tick
**Delete the application data** to also remove your settings and the sign-in entry.

## Using it

Click an item to open its menu; click it again, press <kbd>Esc</kbd> or click anywhere else to
close it. With a menu open, moving along the bar opens the next one. In a menu, the arrow keys
move and <kbd>Enter</kbd> picks.

To quit, open the logo menu and choose **Quit Lintel**. The space it reserved goes back to your
windows.

## Settings

Logo menu → **Menu Bar Settings**. Changes apply and save as you make them, to
`%APPDATA%\Lintel\settings.json`:

- **Appearance**: Auto, light or dark, and the palette
- **Position**: top, bottom, left or right
- **Size**: Compact, Standard or Large
- **Clock**: Windows' format, 12-hour or 24-hour, with or without the date and seconds
- **Show in the bar**: now playing, CPU and memory, volume, network and battery
- **Start Lintel when I sign in**

## Works with Oriel

Lintel is part of the same set of apps as [Oriel](https://github.com/Aduneer/Oriel), the
dashboard for your second screen, and shares its themes. If Oriel is installed, the logo menu
has a shortcut to it. One installer for all of them is on the roadmap.

## FAQ

**My taskbar is at the top too. Where does Lintel go?**
Windows stacks app bars on the same edge, so Lintel sits just below the taskbar. Move one of
them (Lintel to another edge, or the taskbar back to the bottom) for the classic look.

**Why does the network item say "Wi-Fi" but not how strong it is?**
Windows only tells apps the Wi-Fi signal strength when they have location permission. Without
it, Lintel shows the network's name and that you're on Wi-Fi, but no bars.

**It disappeared while I was watching a video.**
Lintel hides while an app is full screen, so games and videos get the whole screen, and comes
back when you leave full screen.

**Does it slow my PC down?**
It updates once a second. A debug build uses about 2% of one CPU core and 140 MB of memory, most
of that for the WebView2 pages that draw the bar and its menus. No admin rights, drivers or
services.

**Does it go online?**
No. Everything comes from Windows itself, on your PC.

**Why isn't it signed?**
Code signing needs a paid certificate and an identity check. It's planned.

## Roadmap

- One installer for Lintel, Oriel and what comes next
- The bar on every screen, not just the main one
- A translucent bar that picks up the wallpaper, like macOS
- Signed releases, then winget

Something you'd like to see? Say so in [Discussions](../../discussions).

## Contributing

Bug reports, ideas and pull requests are welcome: [open an issue](../../issues/new/choose) for a
bug, or start a [discussion](../../discussions) for an idea or a question.
[CONTRIBUTING.md](CONTRIBUTING.md) covers building from source, how Lintel works and working on
the bar in a browser without building the Rust side.

## License

[MIT](LICENSE)

The palettes other than Oriel's own are adapted from [Catppuccin](https://catppuccin.com),
[Nord](https://www.nordtheme.com), [Gruvbox](https://github.com/morhetz/gruvbox),
[Rosé Pine](https://rosepinetheme.com) and [Solarized](https://ethanschoonover.com/solarized/),
all MIT licensed.
