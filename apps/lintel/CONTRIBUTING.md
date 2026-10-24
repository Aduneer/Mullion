# Contributing to Lintel

Thanks for taking a look. Bug reports, ideas and pull requests are all welcome. A menu bar is
on screen all day, so Lintel has to stay light: changes that make it heavier (a bundler, a
framework, a background service, polling something many times a second) need a very good
reason.

## Building from source

You need:

- [Rust](https://rustup.rs)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with
  the **Desktop development with C++** workload
- The WebView2 runtime, which is built into Windows 11 and comes with Edge on Windows 10

Then:

```powershell
cd src-tauri
cargo run
```

The first build takes a few minutes. The app ends up in `src-tauri\target\debug\lintel.exe`,
and **Start Lintel when I sign in** works with that copy too.

Lintel stops its own window from closing (only **Quit Lintel** ends it), so to replace the exe
while it's running, end it first with `taskkill /IM lintel.exe /F`. Windows takes the bar's
space back when the process ends.

> [!NOTE]
> With Smart App Control on, `cargo build --release` may fail with *os error 4551*: Windows won't
> run one of the helper programs Cargo compiles during the build. The `cargo run` debug build
> above works. Please don't turn Smart App Control off to get around this. On most versions of
> Windows it can only be turned back on by reinstalling. Release builds are made by GitHub
> Actions instead (see [Making a release](#making-a-release)).

## How it works

```
src-tauri/            the app: Rust, on Tauri 2 and the system WebView2
  src/main.rs         the bar and menu windows, settings, and the commands the pages can call
  src/appbar.rs       registers the bar as a Windows app bar (SHAppBarMessage) on the edge you pick
  src/apps.rs         which app is in front (a WinEvent hook on its own thread) and its windows
  src/stats.rs        CPU, memory, network, battery and the network's name, once a second
  src/pdh.rs          Windows performance counters
  src/media.rs        what's playing, from Windows' media controls, on its own thread
  src/audio.rs        output and per-app volume via Core Audio, on its own thread
  src/power.rs        lock, sleep, sign out, restart and shut down
  src/settings.rs     the settings file
  src/ffi.rs          the few Win32 functions not taken from the windows crate
  windows/            extra steps for the installer
ui/                   the bar and its menus: plain HTML, CSS and JavaScript, no build step
  themes.css          the palettes, shared with Oriel
  common.js           helpers and icons for both pages
  index.html, bar.*   the bar
  menu.html, menu.*   the menu window, one view per bar item
tools/preview/        runs the bar and its menus in a browser on made-up data
tools/banner.py       draws the README banners from the logo
tools/icons.py        makes the app icons from the logo
assets/               the logo and banners
```

There are two windows. The **bar** is the app bar: it never takes focus, so clicking it doesn't
pull the keyboard away from the window you're in. The **menu** is one popup window that shows
whichever item's menu is open, sizes itself to its content (`menu_ready`) and hides when it
loses focus. A click on the item whose menu just closed that way is treated as "close", not
"open again" (`BLUR_CLICK` in `main.rs`).

The backend pushes a sample to both pages every second over Tauri's IPC, with no local web
server and no open port. While an app is full screen the bar hides and nothing is sampled.

The pages can only ask Windows to open a fixed list of things (Task Manager, File Explorer,
Oriel if it's installed, `ms-settings:` pages and the Wi-Fi list; `run_action` in `main.rs`), act
on windows of the app in front (`window_action`, `quit_app`), control media and volume, and run
the power actions in `power.rs`.

Anything Windows-specific that a test can check without a screen has one (`cargo test`): app bar
geometry, where a menu opens on each edge, settings validation, the Wi-Fi API's struct layout.

## Working on the bar

You don't need the Rust side to work on the bar or its menus. This serves the real `ui/` pages
with a stand-in backend (`tools/preview/mock.js` and `host.js`); edit anything in `ui/` and
reload:

```powershell
python -I tools\preview\serve.py
```

Then open <http://127.0.0.1:5299/tools/preview/>. Options go after the address, for example
`?edge=left&palette=nord&theme=light`, `?open=clock` (a menu open from the start), `?media=off`,
`?battery=off`, `?wifi=off`, `?offline=1`, `?desktop=1` or `?hot=1`. The top of `host.js` lists
them all.

`tools\preview\shots.ps1` renders the README's images again with headless Edge, Chrome or another
Chromium browser, on a fixed clock. With `-Icons` it renders `assets\lintel.svg` and makes the app
icons from it (needs Python with [Pillow](https://pypi.org/project/pillow/)).
`python -I tools\banner.py assets` draws the banners from the logo.

```powershell
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1
```

## Making a release

Releases are built by GitHub Actions (`.github/workflows/release.yml`), not on a PC, so Smart
App Control and local toolchains don't matter:

1. Set `version` in `src-tauri/Cargo.toml` (the installer takes its version from there).
2. Commit, then make a signed tag and push it: `git tag -s v0.1.0 -m "Lintel 0.1.0"` and
   `git push origin v0.1.0`.
3. The workflow builds `Lintel-0.1.0-setup.exe`, `Lintel-0.1.0-portable.zip` and
   `SHA256SUMS.txt`, and drafts a release with them. Add notes and publish it.

The installer is Tauri's NSIS installer. It installs per user, needs no admin rights and, when
"Delete the application data" is ticked on uninstall, also removes the settings and the sign-in
entry (`src-tauri/windows/installer-hooks.nsh`).

Releases aren't code-signed yet, so SmartScreen may warn about them and Smart App Control may
block them. Signing would slot into the workflow's Build step once there's a certificate.
