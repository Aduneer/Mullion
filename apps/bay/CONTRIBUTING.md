# Contributing to Bay

Thanks for taking a look. Bug reports, ideas and pull requests are all welcome. Bay waits all day
for one shortcut and has to answer it the instant it's pressed, so it has to stay light: changes
that make it heavier (a bundler, a framework, a background service, polling anything while the
overview is closed) need a very good reason.

## Building from source

You need:

- [Rust](https://rustup.rs)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with
  the **Desktop development with C++** workload
- The WebView2 runtime, which is built into Windows 11 and comes with Edge on Windows 10

Bay lives in [Mullion](../../README.md)'s repo, with the other apps and the code they share
(`core/`). Clone it, then:

```powershell
cd apps\bay\src-tauri
cargo run
```

The first build takes a few minutes. The app ends up in `target\debug\bay.exe` at the top of the
repo, and **Start at sign-in** works with that copy too.

Bay keeps running with the overview closed (only **Quit Bay** ends it), so to replace the exe
while it's running, end it first with `taskkill /IM bay.exe /F`, or run `bay.exe --quit`.

Two environment variables help when working on the overview: `BAY_MS=2500` slows the windows'
flight down ten times to watch it, and `BAY_TRACE=1` prints how many steps each opening took and
the longest wait between two, and where each window went in the grid.

> [!NOTE]
> With Smart App Control on, `cargo build --release` may fail with *os error 4551*: Windows won't
> run one of the helper programs Cargo compiles during the build. The `cargo run` debug build
> above works. Please don't turn Smart App Control off to get around this. On most versions of
> Windows it can only be turned back on by reinstalling. Release builds are made by GitHub
> Actions instead (see [Making a release](#making-a-release)).

## How it works

```
src-tauri/            the app: Rust, on Tauri 2 and the system WebView2
  src/main.rs         the tray icon, the settings window, the shortcut, and the commands the pages can call
  src/overview.rs     the overview: a window per screen, opening and closing, picking, dragging to another screen
  src/flight.rs       moving the thumbnails, one step per DWM frame
  src/grid.rs         where each window goes in the grid
  src/wallpaper.rs    each screen's wallpaper and how Windows fits it, for the backdrop
  src/settings.rs     the settings file
  src/ffi.rs          the few Win32 functions only Bay uses (the rest are the core's)
  windows/            extra steps for the installer
ui/                   the pages: plain HTML, CSS and JavaScript, no build step
  themes.css          the palettes: a copy of ui-common/themes.css, shared with the other apps
  index.html, bay.*   the overview on one screen: the backdrop, rings, labels, keys and drags
  settings.*          the settings window
tools/preview/        runs the overview and the settings in a browser on made-up windows
assets/               the logo and banners
```

What the apps share is in Mullion's `core/` (the `mullion-core` crate): the Win32 bindings more
than one uses, the Alt+Tab window list (`wins.rs`), shell icons, the global shortcut
(`hotkey.rs`, which Bay and Fanlight both use), live window thumbnails (`thumbs.rs`), the
settings files and the window's look. [Mullion's CONTRIBUTING](../../CONTRIBUTING.md) has more.

### The overview

Bay makes one window per screen when it starts, over that screen's work area (so the taskbar and
Lintel's bar stay in sight), always on top, and keeps them hidden with their pages loaded. The
shortcut (`RegisterHotKey` on a thread of its own) then only has to fill them in and show them:

1. List the windows `wins::enumerate` and `wins::listed` give (what Alt+Tab shows: the current
   desktop's, as windows on other desktops are cloaked), and put each on the screen that has most
   of it (`MonitorFromWindow`).
2. On each screen's window, register a DWM thumbnail of the desktop itself (`Progman`, the
   wallpaper and the icons), cropped to that screen, then one of each window, bottom-most first,
   each exactly over its window. Later thumbnails draw on top, so the screen looks just as it did.
3. Lay out the grid (`grid.rs`), send each page its windows' places, titles and icons, and show
   the windows.
4. Wait two DWM frames, then fly: each step sets every thumbnail's rect with
   `DwmUpdateThumbnailProperties` and waits for DWM's next frame with `DwmFlush`, so the loop runs
   at the screen's refresh rate with no timer, while the desktop's thumbnail fades from opaque to
   gone and the page's backdrop shows through. Each step's place comes from the time gone by, not
   a count of frames, so it stays even when two screens at different rates wake it unevenly.

Closing runs it backwards. A picked window is brought forward first, while Bay is still in front
so Windows allows it, and its thumbnail registered again so it flies back on top; once Bay has
hidden, `land` puts the window on top with the pointer in its middle.

DWM draws thumbnails above everything in Bay's window, the page included, but a thumbnail takes
no input: clicks and keys reach the page underneath, which knows where every thumbnail is. So the
page draws everything *around* them and nothing on them: the backdrop, a ring as an `outline`
with an offset, and each window's icon and title in the strip below it. A badge on a thumbnail
would have to be a second small window above Bay.

The backdrop is the screen's wallpaper, as `IDesktopWallpaper` says it's fitted, served to the
page by a `wallpaper` URI scheme that only hands out the files Windows names, drawn at a tenth of
the size with a blur and stretched back up, under a wash of the palette's colour.

**Minimized windows** have no picture: DWM draws a grey placeholder with the app's icon for
them, which no page can cover. Bay gives them no thumbnail and draws a card with the icon
instead, in the grid with the others, so they're as easy to find and pick, and look like Bay
rather than an old Windows placeholder. **Minimized windows** off in Settings leaves them out.

### Dragging to another screen

A press on a window that moves a few pixels is a drag, and the page hands it to the backend
(`drag`): a page only hears the mouse over its own screen, and a thumbnail only shows inside the
window it's registered with. Each DWM frame the backend reads the pointer, and when it crosses to
another screen, registers the thumbnail there instead. When the button comes up
(`GetAsyncKeyState`, with the buttons swapped if Windows has them swapped), it moves the window
(`place_on`: the same place on the new screen's work area, kept inside, no bigger; a maximized
window is restored, moved and maximized again), lays both screens' grids out again and flies
everything to its new place.

Windows doesn't let a program move the windows of one running as administrator, so Bay checks
for that before a drag starts (`elevated`) and the page says so. Moving windows to another
virtual desktop is left to Windows' Task View (the Desktops button presses Win+Tab once Bay has
closed): the only way to do it for another program's windows is through undocumented COM
interfaces that change between Windows builds.

### What the spike found

Bay started as a spike, in October 2026, to see whether a Tauri window could show and move live
thumbnails of every window smoothly. Debug build, two screens (2560x1080 at 60 Hz and 1920x1080
at 125% and 144 Hz), 15 to 21 windows:

- Updating a thumbnail costs about 50 µs a step, so even 50 windows fit in a 144 Hz frame.
  A 250 ms flight is 21–36 steps.
- `DwmFlush` returns after every composition pass, for every screen: with screens at 60 and
  144 Hz it wakes every 2.5–12 ms. That's why positions come from the time gone by.
- Showing a full-screen window and its WebView's first paint make the first 100–150 ms slow, so
  the overview's windows are made at start-up and kept hidden, and the flight waits for DWM's
  second frame after showing them. The app's openings take 24–34 steps, the longest wait between
  two 13–30 ms.
- Parts of a window hanging off the screen can be unpainted in its thumbnail (GDI programs only
  paint what's visible).
- Everything DWM takes and gives is in physical pixels, so the Rust side stays in them, and each
  page gets its rects divided by its screen's scale.

Anything a test can check without a screen has one (`cargo test`): the grid, where a window goes
on another screen, the flight's easing, the wallpaper's file type, the shortcuts and settings
validation, and in the core, the shortcut's parsing and the thumbnails' rects
(`cargo test --workspace` runs them all).

## Working on the overview

You don't need the Rust side to work on the pages. This serves the real `ui/` pages with a
stand-in backend (`tools/preview/mock.js` and `host.js`) that draws made-up windows where DWM's
thumbnails would be; edit anything in `ui/` and reload:

```powershell
python -I tools\preview\serve.py
```

Then open <http://127.0.0.1:5298/tools/preview/>. Options go after the address, for example
`?sel=2` (→ twice), `?state=flight` (halfway open), `?theme=light&palette=nord`,
`?backdrop=plain` or `?minimized=0`; <kbd>Win</kbd> + <kbd>Ctrl</kbd> + <kbd>↑</kbd> or a
double-click on the stand-in desktop opens and closes it. The top of `host.js` lists them all.
The settings window is at <http://127.0.0.1:5298/ui/settings.html>, with `?mode=dark`,
`?palette=nord` or `?mullion=1`. The windows are made up, so the pictures never show your own PC.
The preview has one screen, so a drag only says it would move the window.

`tools\preview\shots.ps1` renders the README's images and the repo's social preview again with
headless Edge, Chrome or another Chromium browser. With `-Icons` it renders `assets\bay.svg` and
makes the app icons from it (needs Python with [Pillow](https://pypi.org/project/pillow/)).
`python -I tools\banner.py apps\bay`, from the top of the repo, draws the banners from the logo.

```powershell
powershell -ExecutionPolicy RemoteSigned -File tools\preview\shots.ps1
```

## Making a release

Bay is released with the rest of Mullion, at the suite's version, by GitHub Actions:
[Mullion's CONTRIBUTING](../../CONTRIBUTING.md#making-a-release) has the steps. Each release has
`Bay-<version>-setup.exe` and `Bay-<version>-portable.zip` of its own, beside Mullion's
installer.

The installer is Tauri's NSIS installer. It installs per user, needs no admin rights and, when
"Delete the application data" is ticked on uninstall, also removes the settings and the sign-in
entry (`src-tauri/windows/installer-hooks.nsh`).

Releases aren't code-signed yet, so SmartScreen may warn about them and Smart App Control may
block them. Signing would slot into the workflow's Build step once there's a certificate.
