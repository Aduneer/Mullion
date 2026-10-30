# Contributing to Sash

Thanks for taking a look. Bug reports, ideas and pull requests are all welcome. Sash waits all day
for its shortcuts and has to move a window the instant one's pressed, so it has to stay light:
changes that make it heavier (a bundler, a framework, a background service, a keyboard or mouse
hook, polling anything) need a very good reason.

## Building from source

You need:

- [Rust](https://rustup.rs)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with
  the **Desktop development with C++** workload
- The WebView2 runtime, which is built into Windows 11 and comes with Edge on Windows 10

Sash lives in [Mullion](../../README.md)'s repo, with the other apps and the code they share
(`core/`). Clone it, then:

```powershell
cd apps\sash\src-tauri
cargo run
```

The first build takes a few minutes. The app ends up in `target\debug\sash.exe` at the top of the
repo, and **Start at sign-in** works with that copy too.

Sash keeps running with its settings closed (only **Quit Sash** ends it), so to replace the exe
while it's running, end it first with `taskkill /IM sash.exe /F`, or run `sash.exe --quit`.

> [!NOTE]
> With Smart App Control on, `cargo build --release` may fail with *os error 4551*: Windows won't
> run one of the helper programs Cargo compiles during the build. The `cargo run` debug build
> above works. Please don't turn Smart App Control off to get around this. On most versions of
> Windows it can only be turned back on by reinstalling. Release builds are made by GitHub
> Actions instead (see [Making a release](#making-a-release)).

## How it works

```
src-tauri/            the app: Rust, on Tauri 2 and the system WebView2
  src/main.rs         the tray icon, the settings window, the shortcuts, the note, and the commands the pages can call
  src/actions.rs      what Sash can do, each with its shortcut
  src/layout.rs       where a window goes: halves, thirds, quarters, gaps, the next screen (no Win32, all tested)
  src/mover.rs        moving the window in front, on a thread of its own
  src/settings.rs     the settings file
  src/ffi.rs          the few Win32 functions only Sash uses (the rest are the core's)
  windows/            extra steps for the installer
ui/                   the pages: plain HTML, CSS and JavaScript, no build step
  themes.css          the palettes: a copy of ui-common/themes.css, shared with the other apps
  settings.*          the settings window, with its shortcut recorder
  note.*              the note saying why a window didn't move
tools/preview/        runs a made-up desktop, the settings and the note in a browser
assets/               the logo and banners
```

What the apps share is in Mullion's `core/` (the `mullion-core` crate): the Win32 bindings more
than one uses, the Alt+Tab window list (`wins.rs`), the global shortcuts (`hotkey.rs`, which Sash,
Bay and Fanlight use), where a window really is (`thumbs::bounds`), the settings files and the
window's look. [Mullion's CONTRIBUTING](../../CONTRIBUTING.md) has more.

### A shortcut, start to finish

1. Every action's shortcut is registered with `RegisterHotKey` on the core's shortcut thread
   (`hotkey::Hotkeys`, one id per action). Nothing else listens to the keyboard.
2. A press sends a job to the mover's thread, so a program that's slow to answer never holds up
   the shortcuts or the settings window.
3. The mover takes the window in front (`GetForegroundWindow`), and only if Alt+Tab would list it
   (`wins::listed`) and it isn't minimized; a window that isn't responding (`IsHungAppWindow`)
   is left alone, and the note says so.
4. Its screen's work area and scaling come from `MonitorFromWindow`, `GetMonitorInfoW` and
   `GetDpiForMonitor`. `layout.rs` works out where it goes, in physical pixels.
5. The window is put there with `SetWindowPos` (see below), restored first if it's maximized.

### Where a window goes

The work area is cut into sixths each way: that gives halves, thirds and two-thirds, and a place
is a span across and one down. Every place's edges come from the same rounding, so two windows
side by side share an edge exactly, or are exactly the gap apart (a full gap at the screen's
edges, half from each window between them).

Sash keeps no state for the arrows: it looks at where the window is now (`layout::detect`, to
within 3 pixels, since some programs round their size) and decides from that (`layout::step`).
So a window snapped by Windows, or put on a half by hand, cycles on just the same.

### Exact edges, at any scaling

Windows 10 and 11 give most windows invisible borders, about 7 pixels at 100%, to grab for
resizing. `GetWindowRect` includes them; what you see is DWM's extended frame bounds
(`DWMWA_EXTENDED_FRAME_BOUNDS`). Sash measures the difference on the window as it is and asks
`SetWindowPos` for the rect that puts what's seen exactly on the place.

Moved onto a screen with different scaling, a window rescales itself (`WM_DPICHANGED`), borders
and all, and some programs do that a moment later. So after a move between screens of different
scaling, Sash waits 60 ms and places it again. A window that won't take the size (a smallest size,
or a fixed one) is then moved to keep to its place's side of the screen (`layout::fit`).

Everything is in physical pixels: Sash is per-monitor DPI aware (Tauri makes it so), and DWM's
bounds are physical anyway.

### Windows it can't move

User Interface Privilege Isolation stops a normal program from moving the windows of one running
as administrator, with *access denied*. Sash doesn't look into other programs to find out:
before it changes anything it asks Windows to redraw the window's frame where it is
(`SetWindowPos` with `SWP_FRAMECHANGED` and nothing else), which is refused the same way. Then a
small note (`note.html`) says so at the bottom of the screen. The note's window is made hidden,
sized to the page once it has drawn, shown with `SW_SHOWNOACTIVATE` so the window in front keeps
the focus, and dropped after a few seconds.

### Choosing a shortcut

The settings page records a shortcut from the key's place on the keyboard (`KeyboardEvent.code`),
so AltGr+E is "ctrl+alt+e" rather than "€". While it listens, the backend lets go of all of Sash's
shortcuts (`recording`), or Sash would catch them before the page could; `set_shortcut` takes them
back, registers the new one, and keeps it only if Windows does.

Ctrl+Alt with a letter or digit is AltGr with that key on many keyboards, which types a character,
so none of the defaults is one, and the page says so when you pick one.

Anything a test can check without a screen has one (`cargo test`): every place, gap, the arrows'
steps, centring, carrying a window to another screen, a window too big for its place, the order of
screens, the actions' defaults and settings validation; in the core, the shortcuts' parsing
(`cargo test --workspace` runs them all).

## Working on the pages

You don't need the Rust side to work on the pages. This serves the real `ui/` pages with a
stand-in backend (`tools/preview/mock.js`), and a made-up desktop (`host.js`) whose windows move
by the same rules as `layout.rs`; edit anything in `ui/` and reload:

```powershell
python -I tools\preview\serve.py
```

Then open <http://127.0.0.1:5293/tools/preview/>, click a window and press <kbd>Ctrl</kbd> +
<kbd>Alt</kbd> and an arrow, <kbd>Enter</kbd>, <kbd>Space</kbd> or <kbd>Backspace</kbd>. Options go
after the address, for example `?scene=thirds`, `?mode=light`, `?gap=8`, `?keys=ctrl+alt+left`
or `?note=1`; the top of `host.js` lists them all. The settings window is at
<http://127.0.0.1:5293/ui/settings.html>, with `?mode=dark`, `?palette=nord`, `?mullion=1` or
`?taken=1`. The windows are made up, so the pictures never show your own PC.

`tools\preview\shots.ps1` renders the README's images and the repo's social preview again with
headless Edge, Chrome or another Chromium browser. With `-Icons` it renders `assets\sash.svg` and
makes the app icons from it (needs Python with [Pillow](https://pypi.org/project/pillow/)).
`python -I tools\banner.py apps\sash`, from the top of the repo, draws the banners from the logo.

```powershell
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1
```

## Making a release

Sash is released with the rest of Mullion, at the suite's version, by GitHub Actions:
[Mullion's CONTRIBUTING](../../CONTRIBUTING.md#making-a-release) has the steps. Each release has
`Sash-<version>-setup.exe` and `Sash-<version>-portable.zip` of its own, beside Mullion's
installer.

The installer is Tauri's NSIS installer. It installs per user, needs no admin rights and, when
"Delete the application data" is ticked on uninstall, also removes the settings and the sign-in
entry (`src-tauri/windows/installer-hooks.nsh`).

Releases aren't code-signed yet, so SmartScreen may warn about them and Smart App Control may
block them. Signing would slot into the workflow's Build step once there's a certificate.
