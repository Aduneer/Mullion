# Contributing to Sill

Thanks for taking a look. Bug reports, ideas and pull requests are all welcome. Sill sits in the
taskbar's place all day and answers every app that puts an icon in the notification area, so it
has to stay light and it must never hang: changes that make it heavier (a bundler, a framework, a
background service, polling quickly) or that let its tray window wait on anyone need a very good
reason.

## Building from source

You need:

- [Rust](https://rustup.rs)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with
  the **Desktop development with C++** workload
- The WebView2 runtime, which is built into Windows 11 and comes with Edge on Windows 10

Sill lives in [Mullion](../../README.md)'s repo, with the other apps and the code they share
(`core/`). Clone it, then:

```powershell
cd apps\sill\src-tauri
cargo build
..\..\..\target\debug\sill.exe
```

The first build takes a few minutes. It makes two programs in `target\debug` at the top of the
repo: `sill.exe`, and `sill-guard.exe`, which puts the taskbar back if `sill.exe` stops without
doing it. Build Fanlight too (`cargo build -p fanlight`) for the apps grid: Sill finds it beside
its own exe.

Sill keeps running with its settings closed (only **Quit Sill** ends it), so to replace the exe
while it's running, run `sill.exe --quit`, which puts the taskbar back as it quits. If you end it
with `taskkill /F`, the guard puts the taskbar back a moment later; if that's gone too,
`sill.exe --restore-taskbar` does.

> [!NOTE]
> With Smart App Control on, a build may fail with *os error 4551*: Windows won't run one of the
> programs Cargo just compiled. `cargo clean -p <the crate it names>` and building again usually
> gets past it. Please don't turn Smart App Control off to get around this. On most versions of
> Windows it can only be turned back on by reinstalling. Release builds are made by GitHub Actions
> instead (see [Making a release](#making-a-release)).

## How it works

```
src-tauri/            the app: Rust, on Tauri 2 and the system WebView2
  src/main.rs         the tray icon, the dock and settings windows, and the commands pages call
  src/dock.rs         the dock's window: an app bar in the taskbar's place, or floating above it
  src/taskbar.rs      hiding Windows' taskbar, and putting it back
  src/host.rs         the notification area's host: the window apps send their icons to
  src/trayicons.rs    those icons as pictures, and which ones Windows keeps out of sight
  src/pins.rs         the pinned apps, and the taskbar's, to start with
  src/menu.rs         the dock's right-click menus, as native popup menus
  src/settings.rs     the settings file
  src/guard.rs        sill-guard.exe: waits for Sill to end, and puts the taskbar back if it must
  src/ffi.rs          the Win32 functions only Sill uses (the rest are the core's)
  windows/            extra steps for the installer
ui/                   the pages: plain HTML, CSS and JavaScript, no build step
  themes.css          the palettes: a copy of ui-common/themes.css, shared with the other apps
  dock.*              the dock
  settings.*          the settings window
tools/preview/        runs the pages, on a made-up desktop, in a browser
assets/               the logo and banners
```

What the apps share is in Mullion's `core/` (the `mullion-core` crate): the running apps and
their windows (`apps.rs`, which Lintel's bar uses too), app icons, the wallpaper for glass, how
the apps find and start each other (`suite.rs`), the settings files and the window's look.
[Mullion's CONTRIBUTING](../../CONTRIBUTING.md) has more.

### The taskbar's place

Starting, Sill registers the dock as an app bar on the main screen's edge (`SHAppBarMessage`,
`ABM_NEW` and `ABM_SETPOS`), so maximized windows stop at it, then hides the taskbar: every
`Shell_TrayWnd` and `Shell_SecondaryTrayWnd` hidden, and auto-hide set (`ABM_SETSTATE`) so
Explorer gives back its strip of the screen. The state it found is saved in
`%APPDATA%\Sill\taskbar-state` first; the file is how anything after can tell Sill hid the
taskbar and hasn't put it back. A keeper on a thread of its own checks once a second that the
taskbar is still hidden and still auto-hide, and that the main screen's work area stops at the
dock (Explorer sometimes leaves it where the taskbar was).

Quitting puts all of that back and deletes the file. Starting, Sill starts `sill-guard.exe` beside
it, which waits for Sill's process to end and, if the file is still there, runs
`sill.exe --restore-taskbar`. That flag is handled before anything else starts, does nothing if
the file isn't there, and is also what the uninstallers run.

With **Float and auto-hide** on, none of that happens: no app bar, the taskbar untouched, and the
dock's window is just the shelf, topmost. A thread looks where the pointer is ten times a second,
only in this mode, and the dock slides in after the pointer has been at the edge for a moment
(the corners are Mitre's, so they don't count) and away once it's left the dock.

### The notification area

Apps put their icons in the notification area by sending `WM_COPYDATA` to the window of class
`Shell_TrayWnd` (`Shell_NotifyIcon` finds it with `FindWindow`). Sill makes a window of that class
of its own, topmost so it's the one found, on a thread of its own, and:

- answers each icon message (`dwData` 1) from its own table at once, and passes it on to
  Explorer's taskbar from another thread, in order, so Explorer keeps its own table and takes over
  if Sill quits. App-bar (`dwData` 0) and icon-position (3) messages are passed on as they come,
  with `SMTO_ABORTIFHUNG`. The host window never waits on anyone: a hung Explorer or app must not
  hang every app that adds an icon.
- broadcasts `TaskbarCreated` as it starts, so apps add their icons again, now to Sill; and again
  a few seconds later for slow ones. The re-adds that follow its own broadcast aren't passed on to
  Explorer (it has them), but Explorer's own `TaskbarCreated`, when it restarts, has Sill hide the
  new taskbar and send it Sill's whole table.
- leaves out Explorer's own icons (volume, network, power): they're drawn by the taskbar itself.
- drops an icon whose window is gone, every few seconds.

A click on an icon is sent to its window as the taskbar would send it: its callback message with
the mouse message (and `NIN_SELECT` or `WM_CONTEXTMENU` for icons that ask for version 4), after
`AllowSetForegroundWindow`, so its menu or window can come to the front. Which icons are out of
sight under **^** is what Windows keeps under
`HKEY_CURRENT_USER\Control Panel\NotifyIconSettings` (`IsPromoted`).

### The dock

The running apps come from the core's `apps.rs`: WinEvent hooks say when windows come and go, so
nothing is polled. Pinned apps are kept by program path (or app ID) and the shortcut or command
that opens them. The first time it starts, Sill reads the shortcuts in
`%APPDATA%\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar` (`IShellLinkW`) to pin
what the taskbar had.

Right-click menus are native popup menus (`TrackPopupMenuEx`), dark when the theme is, so they
can open beyond the dock's window.

Anything a test can check without a screen has one (`cargo test`): the icon messages' layout, the
table's merging and pruning, where the dock goes on each edge and floating, settings validation,
merging pins with running apps, and the work-area check; in the core, the running apps' grouping
and `suite::runs` (`cargo test --workspace` runs them all).

## Working on the pages

You don't need the Rust side to work on the pages. This serves the real `ui/` pages with a
stand-in backend (`tools/preview/mock.js`); edit anything in `ui/` and reload:

```powershell
python -I tools\preview\serve.py
```

Then open <http://127.0.0.1:5290/tools/preview/>, a made-up desktop with the dock along its
bottom, with `?mode=dark`, `?palette=nord`, `?edge=left`, `?size=large`, `?float=1`, `?glass=1`
or `?fanlight=0`; the top of `mock.js` lists them all. What the dock asks for (a menu, a click on
an icon) shows in the top right. <http://127.0.0.1:5290/ui/settings.html> is the settings window,
with `?mullion=1` or `?look=1` too. The apps and icons are made up, so the pictures never show your
own PC.

`tools\preview\shots.ps1` renders the README's images and the repo's social preview again with
headless Edge, Chrome or another Chromium browser. With `-Icons` it renders `assets\sill.svg` and
makes the app icons from it (needs Python with [Pillow](https://pypi.org/project/pillow/)).
`python -I tools\banner.py apps\sill`, from the top of the repo, draws the banners from the logo.

```powershell
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1
```

## Making a release

Sill is released with the rest of Mullion, at the suite's version, by GitHub Actions:
[Mullion's CONTRIBUTING](../../CONTRIBUTING.md#making-a-release) has the steps. Each release has
`Sill-<version>-setup.exe` and `Sill-<version>-portable.zip` of its own, beside Mullion's
installer; both carry `sill-guard.exe` with `sill.exe`.

The installer is Tauri's NSIS installer. It installs per user, needs no admin rights, quits Sill
before installing or uninstalling, runs `sill.exe --restore-taskbar` before uninstalling and,
when "Delete the application data" is ticked, also removes the settings and the sign-in entry
(`src-tauri/windows/installer-hooks.nsh`).

Releases aren't code-signed yet, so SmartScreen may warn about them and Smart App Control may
block them. Signing would slot into the workflow's Build step once there's a certificate.
