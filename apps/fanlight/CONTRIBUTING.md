# Contributing to Fanlight

Thanks for taking a look. Bug reports, ideas and pull requests are all welcome. Fanlight runs all
day and has to open the instant you press the shortcut, so it has to stay light: changes that make
it heavier (a bundler, a framework, a background service, polling something many times a second,
reading files before they're asked for) need a very good reason.

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

The first build takes a few minutes. The app ends up in `src-tauri\target\debug\fanlight.exe`,
and **Start at sign-in** works with that copy too.

Fanlight keeps running with its panel closed (only **Quit Fanlight** ends it), so to replace the
exe while it's running, end it first with `taskkill /IM fanlight.exe /F`.

> [!NOTE]
> With Smart App Control on, `cargo build --release` may fail with *os error 4551*: Windows won't
> run one of the helper programs Cargo compiles during the build. The `cargo run` debug build
> above works. Please don't turn Smart App Control off to get around this. On most versions of
> Windows it can only be turned back on by reinstalling. Release builds are made by GitHub
> Actions instead (see [Making a release](#making-a-release)).

## How it works

```
src-tauri/            the app: Rust, on Tauri 2 and the system WebView2
  src/main.rs         the panel's window, the tray icon, settings, and the commands the page can call
  src/search.rs       one search: every source scored on one scale, the Top Hit and the sections
  src/matcher.rs      how well a name matches what was typed
  src/files.rs        the file index, kept in memory and brought up to date folder by folder
  src/shell.rs        the Start menu's apps, and icons and thumbnails, on a COM thread of its own
  src/icons.rs        an icon or thumbnail from the shell, as an image the page can show
  src/preview.rs      what the preview shows: a file's start, a folder's items, the facts
  src/calc.rs         sums and unit conversions
  src/catalog.rs      the commands and the Settings pages, with the words they go by
  src/clipboard.rs    the clipboard history (a clipboard format listener, nothing polled)
  src/history.rs      what you've opened, so it ranks higher, and the searches for ↑
  src/wins.rs         the open windows, as Alt+Tab lists them
  src/hotkey.rs       the shortcut (RegisterHotKey on a thread of its own)
  src/power.rs        lock, sleep, sign out, restart and shut down
  src/settings.rs     the settings file
  src/ffi.rs          the few Win32 functions not taken from the windows crate
  windows/            extra steps for the installer
ui/                   the panel: plain HTML, CSS and JavaScript, no build step
  themes.css          the palettes, shared with Oriel and Lintel
  panel.*             the search field, the results, the preview and the settings
tools/preview/        runs the panel in a browser on made-up data
tools/banner.py       draws the README banners from the logo
tools/icons.py        makes the app icons from the logo
assets/               the logo and banners
```

There's one window, the panel, and it's exactly as big as what's in it: the page measures itself
and tells the backend (`fit` in `main.rs`), so Windows can round its corners, give it a shadow
and, with glass, blur what's behind it (acrylic). It widens about its middle for the preview.
The shortcut shows it on the screen the pointer is on, a little above the middle, and gives the
focus back to the window you were in when it closes.

Each keystroke asks the backend to search (`search`), numbered so that a slow answer to an older
query is dropped. Everything is searched every time, in memory: the apps, the catalog, the open
windows, the clipboard history and the file index (split across up to eight threads). All
the scores are on one scale, so the best of them all can be the Top Hit; each kind has a weight,
and what you picked before adds to it (`history.rs`). Icons come afterwards, one by one, as the
shell draws them.

The file index (`files.rs`) remembers each folder's last-modified time. Windows changes that time
when a file in the folder is added, removed or renamed, so when the panel opens, only the folders
that changed are read again. It reads names and attributes only, never what's in a file; the
preview reads the start of one file when you ask for it.

The page can only open what the backend gave it. `open` checks a result's key before acting on
it: an app must be one the Start menu lists, a file must be there, a page or command must be in
the catalog, a window must still belong to the process it did. [SECURITY.md](SECURITY.md) has the
full list.

Anything that a test can check without a screen has one (`cargo test`): the matcher, the sums and
units, the index and how it updates, the search's ranking and categories, the previews, settings
validation, icons.

## Working on the panel

You don't need the Rust side to work on the panel. This serves the real `ui/` page with a
stand-in backend (`tools/preview/mock.js` and `host.js`); edit anything in `ui/` and reload:

```powershell
python -I tools\preview\serve.py
```

Then open <http://127.0.0.1:5297/tools/preview/>. Options go after the address, for example
`?q=lisbon&sel=1&preview=1` (typed in, ↓ once, then Space), `?only=clipboard`,
`?palette=nord&theme=light`, `?glass=on`, `?apps=list` or `?view=settings`. The top of `host.js` lists them
all. The apps, files, windows and clipboard are made up, so the pictures never show your own PC.
The stand-in searches roughly as `search.rs` does; it's there for the look, so check ranking in
the app or with `cargo test`.

`tools\preview\shots.ps1` renders the README's images and the repo's social preview again with
headless Edge, Chrome or another Chromium browser. With `-Demo` it records the animation at the
top of the README (`docs/images/demo.webp`) from `?demo=1` (`demo.py`), which needs
[ffmpeg](https://ffmpeg.org) (`winget install Gyan.FFmpeg`). With `-Icons` it renders
`assets\fanlight.svg` and makes the app icons from it (needs Python with
[Pillow](https://pypi.org/project/pillow/)).
`python -I tools\banner.py assets` draws the banners from the logo.

```powershell
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1
```

## Making a release

Releases are built by GitHub Actions (`.github/workflows/release.yml`), not on a PC, so Smart
App Control and local toolchains don't matter:

1. Set `version` in `src-tauri/Cargo.toml` (the installer takes its version from there).
2. Commit, then make a signed tag and push it: `git tag -s v0.1.0 -m "Fanlight 0.1.0"` and
   `git push origin v0.1.0`.
3. The workflow builds `Fanlight-0.1.0-setup.exe`, `Fanlight-0.1.0-portable.zip` and
   `SHA256SUMS.txt`, and drafts a release with them. Add notes and publish it.

The installer is Tauri's NSIS installer. It installs per user, needs no admin rights and, when
"Delete the application data" is ticked on uninstall, also removes the settings, the history and
the sign-in entry (`src-tauri/windows/installer-hooks.nsh`).

Releases aren't code-signed yet, so SmartScreen may warn about them and Smart App Control may
block them. Signing would slot into the workflow's Build step once there's a certificate.
