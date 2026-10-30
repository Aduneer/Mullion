# Contributing to Mitre

Thanks for taking a look. Bug reports, ideas and pull requests are all welcome. Mitre listens to
the pointer all day, so it has to stay light: changes that make it heavier (a bundler, a
framework, a background service, a mouse or keyboard hook, polling the pointer quickly) need a
very good reason.

## Building from source

You need:

- [Rust](https://rustup.rs)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with
  the **Desktop development with C++** workload
- The WebView2 runtime, which is built into Windows 11 and comes with Edge on Windows 10

Mitre lives in [Mullion](../../README.md)'s repo, with the other apps and the code they share
(`core/`). Clone it, then:

```powershell
cd apps\mitre\src-tauri
cargo run
```

The first build takes a few minutes. The app ends up in `target\debug\mitre.exe` at the top of the
repo, and **Start at sign-in** works with that copy too. Build Bay and Fanlight too
(`cargo build -p bay -p fanlight`) for their corners: Mitre finds them beside its own exe.

Mitre keeps running with its settings closed (only **Quit Mitre** ends it), so to replace the exe
while it's running, end it first with `taskkill /IM mitre.exe /F`, or run `mitre.exe --quit`.

> [!NOTE]
> With Smart App Control on, a build may fail with *os error 4551*: Windows won't run one of the
> programs Cargo just compiled. `cargo clean -p mitre` and building again usually gets past it.
> Please don't turn Smart App Control off to get around this. On most versions of Windows it can
> only be turned back on by reinstalling. Release builds are made by GitHub Actions instead (see
> [Making a release](#making-a-release)).

## How it works

```
src-tauri/            the app: Rust, on Tauri 2 and the system WebView2
  src/main.rs         the tray icon, the settings window, and the commands the page can call
  src/watch.rs        hearing the pointer reach a corner, on a thread of its own
  src/corners.rs      the screens and their corners (the geometry is tested without a screen)
  src/glow.rs         the glow in the corner: a native layered window, drawn a pixel at a time
  src/actions.rs      what a corner can do
  src/settings.rs     the settings file
  src/ffi.rs          the Win32 functions only Mitre uses (the rest are the core's)
  windows/            extra steps for the installer
ui/                   the page: plain HTML, CSS and JavaScript, no build step
  themes.css          the palettes: a copy of ui-common/themes.css, shared with the other apps
  settings.*          the settings window, with each screen and its corners' menus
tools/preview/        runs the settings, and a made-up desktop with the glow, in a browser
assets/               the logo and banners
```

What the apps share is in Mullion's `core/` (the `mullion-core` crate): the Win32 bindings more
than one uses, how the apps find and start each other (`suite.rs`), the power commands, the
settings files and the window's look. [Mullion's CONTRIBUTING](../../CONTRIBUTING.md) has more.

### A corner, start to finish

1. `watch.rs` makes a hidden window on a thread of its own and registers it for Raw Input from
   the mouse (`RegisterRawInputDevices`, usage page 1, usage 2, `RIDEV_INPUTSINK` so it hears the
   mouse whatever's in front). It never reads the raw data: each `WM_INPUT` only calls
   `GetCursorPos`. So Mitre wakes only while the mouse moves, and costs nothing to speak of then.
   A low-level mouse hook (`WH_MOUSE_LL`) would see every mouse event first and look like spyware
   to Smart App Control; polling would wake it all day.
2. `corners::corner_at` says whether the pointer is within a few pixels of a screen's corner
   (more on a screen scaled up). Screens are in physical pixels (Mitre is per-monitor DPI aware,
   as Tauri makes it), so mixed scaling and screens left of or above the main one need nothing
   special.
3. Arriving in a corner that does something starts a one-shot timer for the wait: a pointer
   resting in the corner brings no more messages, so only a timer can tell it stayed. The glow
   starts growing. If a mouse button is held (a drag) or a full-screen app is in front, it's
   that visit's loss: nothing grows and nothing fires until the pointer leaves and comes back.
4. When the timer ends, the pointer has to be in the same corner still, with no button held and
   nothing full screen. Then the glow flashes, and `main.rs` runs the corner's action on another
   thread, so the pointer's thread listens again at once.
5. Leaving the corner before then stops the timer and fades the glow.

Raw Input once went quiet for half a minute in the spike while the pointer kept moving (an
administrator's window in front is the likely cause: Windows doesn't pass its input to a normal
program). So a 1 s timer stands in: if the pointer moved and Raw Input said nothing, it looks
where the pointer is anyway.

The window is a hidden top-level one rather than message-only, because only top-level windows
hear `WM_DISPLAYCHANGE`: screens plugged in, moved or rescaled re-read the list, and the
settings page gets the new one.

### Full screen

The window in front counts as full screen, for its own screen only, when it has no title bar and
covers that screen (a maximized window with the taskbar hidden, as Lintel can, covers it too, but
has a title bar). `SHQueryUserNotificationState` adds games running Direct3D full screen and
presentations, for every screen. Explorer's windows (the desktop, Task View, Start) and the suite's
own (Bay's overview) never count, so pushing a corner again can close them.

### Push again to close

Mitre doesn't guess whether Bay's overview or Fanlight's panel is open: it starts their exe with
`--toggle` (`suite::Ask::Toggle`), which reaches the running app as any second start does, and the
app opens, or closes if it's open. Fanlight takes `--apps` with it, to open on its Apps grid.
Win+D and Win+Tab close themselves.

### Screens and their corners

Each screen's corners are kept under its monitor's device path (`\\?\DISPLAY#GSM5B7F#…`, from
`EnumDisplayDevicesW`), which names the monitor rather than Windows' numbering of its outputs, so
they stay with the monitor from one start to the next. A corner where another screen is past
either edge lets the pointer slide through it (`corners::is_hard`); the settings say which.

### The glow

A layered window (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE`, topmost), so clicks
pass through and it never takes the focus, drawn into a 32-bit bitmap a pixel at a time and put on
screen with `UpdateLayeredWindow`: a quarter-circle of the palette's accent, half way to white,
fading out from the corner. A 15 ms timer on the pointer's thread draws its frames, only while
it's on screen. `tools/preview/host.js` draws it the same way, to look at in a browser.

Anything a test can check without a screen has one (`cargo test`): the corners of one, two and
three screens, inner corners, the actions, which programs never count as full screen, and
settings validation; in the core, the `--toggle` ask (`cargo test --workspace` runs them all).

## Working on the page

You don't need the Rust side to work on the settings page. This serves the real `ui/` page with
a stand-in backend (`tools/preview/mock.js`); edit anything in `ui/` and reload:

```powershell
python -I tools\preview\serve.py
```

Then open <http://127.0.0.1:5292/ui/settings.html>, with `?mode=dark`, `?palette=nord`,
`?screens=1` (or `3`), `?missing=Bay,Fanlight`, `?mullion=1` or `?look=1`; the top of `mock.js`
lists them all. <http://127.0.0.1:5292/tools/preview/> is a made-up desktop: push the mouse into a
corner of the page and wait, and the glow grows there as Mitre's does, with `?corner=tl` (or `tr`,
`bl`, `br`) to hold it for a picture and `?palette=nord` for its colour. The screens are made up,
so the pictures never show your own PC.

`tools\preview\shots.ps1` renders the README's images and the repo's social preview again with
headless Edge, Chrome or another Chromium browser. With `-Icons` it renders `assets\mitre.svg` and
makes the app icons from it (needs Python with [Pillow](https://pypi.org/project/pillow/)).
`python -I tools\banner.py apps\mitre`, from the top of the repo, draws the banners from the logo.

```powershell
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1
```

## Making a release

Mitre is released with the rest of Mullion, at the suite's version, by GitHub Actions:
[Mullion's CONTRIBUTING](../../CONTRIBUTING.md#making-a-release) has the steps. Each release has
`Mitre-<version>-setup.exe` and `Mitre-<version>-portable.zip` of its own, beside Mullion's
installer.

The installer is Tauri's NSIS installer. It installs per user, needs no admin rights and, when
"Delete the application data" is ticked on uninstall, also removes the settings and the sign-in
entry (`src-tauri/windows/installer-hooks.nsh`).

Releases aren't code-signed yet, so SmartScreen may warn about them and Smart App Control may
block them. Signing would slot into the workflow's Build step once there's a certificate.
