# Contributing to Mullion

Thanks for taking a look. Bug reports, ideas and pull requests are all welcome. Mullion's apps run
all day beside everything else you do, so they have to stay light: changes that make one heavier
(a bundler, a framework, a background service, polling something many times a second) need a very
good reason.

This file covers the repo as a whole. Each app's own CONTRIBUTING, in `apps/<app>/`, covers
building it, how it works and working on its pages in a browser.

## How the repo fits together

```
Cargo.toml            the workspace: the core and every app, one lockfile, one target/
core/                 mullion-core, what the apps share (below)
apps/fanlight/        Fanlight: its Rust side (src-tauri/), pages (ui/), preview (tools/preview/),
                      logo and banners (assets/), screenshots (docs/), and its own README,
                      CONTRIBUTING and SECURITY
apps/lintel/          Lintel, laid out the same way
apps/oriel/           Oriel, laid out the same way
apps/bay/             Bay, laid out the same way
apps/mullion/         Mullion.exe: the suite's tray icon, sign-in entry and settings window (below)
ui-common/            the page files every app shares word for word (themes.css)
tools/                scripts every app's preview uses: icons.py, preview/demo.py and
                      preview/Shots.psm1 (the browser, screenshots, icons, social preview),
                      and banner.py, which draws each app's README banners from its logo
installer/            mullion.nsi, Mullion's installer, and its pictures
.github/              CI, the release workflow, the issue forms
```

Each app is its own exe and process, built from its own crate, and works without the others.
They share code at build time only.

### The core

`mullion-core` has what more than one app needs, so each isn't carrying its own copy:

```
src/ffi.rs            the hand-written Win32 bindings more than one app uses, and their helpers
src/settings.rs       each app's folder in %APPDATA%, the palettes, and reading and writing JSON
                      files safely (a temporary file, then renamed over the old one)
src/icons.rs          an icon or thumbnail from the shell, as an image a page can show
src/wins.rs           the windows Alt+Tab would list, and bringing one to the front
src/thumbs.rs         DWM's live thumbnails of those windows, and where each window really is
src/hotkey.rs         a global shortcut on a thread of its own, changed without restarting
src/power.rs          lock, sleep, sign out, restart and shut down
src/look.rs           following the theme, Windows 11's rounded corners, the WebView2 flags
src/suite.rs          the apps Mullion has, finding each one's exe, and the flags Mullion.exe
                      starts them with
src/pdh.rs            Windows performance counters (the `pdh` feature)
src/media.rs          what's playing, from Windows' media controls (the `media` feature)
src/audio.rs          output and per-app volume via Core Audio (the `audio` feature)
```

The last three are behind Cargo features, so an app that doesn't use them doesn't compile them
or the windows crate's parts they need; Oriel and Lintel turn all three on.

Each app keeps a small `ffi.rs` of its own for the bindings only it uses; it starts with
`pub use mullion_core::ffi::*`, so the app calls everything as `ffi::…`. A binding moves into the
core when a second app needs it.

A change to the core is a change to every app that uses it: build and try each of them, and run
the whole workspace's tests and lints, as CI does:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Mullion.exe and the apps

Installed with Mullion's installer, the apps sit beside `mullion.exe` in one folder, and Mullion
starts each one that's on with `--mullion`. An app started that way leaves its tray icon and
sign-in entry to Mullion, and shows "see Mullion Settings" where its own sign-in switch was.
Mullion asks things of a running app by starting its exe again with `--show`, `--settings` or
`--quit`: the single-instance plugin hands those to the app that's running, as it hands over any
second start. Started any other way, an app runs on its own, exactly as before. `suite.rs` has
all of it, and each app's `main.rs` acts on it in its single-instance callback and in `setup`.

Mullion keeps no WebView while its settings window is closed: the window is made when it opens
and dropped when it closes.

To try it, build the apps and Mullion (`cargo build --workspace`) and run
`target\debug\mullion.exe`; it finds the apps beside it.

### Adding an app

1. `apps/<app>/`, laid out like the others, and its `src-tauri` in the workspace's `members`.
2. An entry in `APPS` in `core/src/suite.rs`, and in its `main.rs`, the same `--mullion` handling
   as Fanlight's (no tray icon of its own, no sign-in entry, and `--show`, `--settings` and
   `--quit`). Its icon in `icon_of` in `apps/mullion/src-tauri/src/main.rs`, which builds
   each app's icon into Mullion.
3. In `installer/mullion.nsi`: an `!insertmacro App` line, its line in the `-Tidy` section and
   `.onInit`, an `UnData` line and a `MUI_DESCRIPTION_TEXT` line.
4. It in the root README's table and SECURITY.md, and the bug form's list of apps.
5. Its first-run hint in `FIRST` in `apps/mullion/ui/settings.js`, its row in Mullion's preview
   (`apps/mullion/tools/preview/mock.js`), and its name and colour in `tools/banner.py`. Then
   draw the pictures again: the banners (`python -I tools\banner.py apps\<app>` and
   `apps\mullion`), Mullion's settings screenshots, and the installer's pictures
   (`apps\mullion\tools\preview\shots.ps1 -Installer`), whose logos and colours list every app.
6. Each other app's "Part of Mullion" paragraph, and Fanlight's About line, name it too.

### The shared page files

There's no bundler, so an app's pages can only load files in its own `ui/`. Each app keeps a copy
of `ui-common/themes.css` as `ui/themes.css`, which its `build.rs` refreshes on every build. Edit
the one in `ui-common/`, build, and commit the copies with it; CI fails if a copy differs.

> [!NOTE]
> With Smart App Control on, a fresh build can fail with *os error 4551* or "can't find crate":
> Windows won't run a helper program Cargo just compiled, or load a macro library. Building again
> usually gets past it (for a macro library, `cargo clean -p <crate>` first). Please don't turn
> Smart App Control off: on most versions of Windows it can only be turned back on by
> reinstalling. Release builds are made by GitHub Actions instead.

## Making a release

The suite has one version, and every release has every app. Releases are built by GitHub Actions
(`.github/workflows/release.yml`), not on a PC, so Smart App Control and local toolchains don't
matter:

1. Set `version` in every app's `src-tauri/Cargo.toml` (each installer takes its version from
   there).
2. Commit, then make a signed tag and push it: `git tag -s v0.3.0 -m "Mullion 0.3.0"` and
   `git push origin v0.3.0`.
3. The workflow builds each app's `<App>-0.3.0-setup.exe` and `<App>-0.3.0-portable.zip`, tries
   each installer on a clean Windows, builds `Mullion-0.3.0-setup.exe` and
   `Mullion-0.3.0-portable.zip` (Mullion.exe and every app) and tries the installer too, and
   drafts a release with them all and `SHA256SUMS.txt`. Add notes and publish it.

Each app's installer is Tauri's NSIS installer: per user, no admin rights, and its own uninstaller.
Mullion's installer (`installer/mullion.nsi`) is its own NSIS script: a box for each app, every
ticked app's exe beside `mullion.exe` in one folder, and one uninstaller for them all. It can only
be built in Actions, with the NSIS that Tauri's build downloads; run the release workflow by hand
for a dry run that builds and tries everything without releasing. Its sidebar and header pictures
(`installer/*.bmp`) come from `apps\mullion\tools\preview\shots.ps1 -Installer`.

Its uninstaller runs in place (`_?=` in its Installed apps entry): started the usual NSIS way, it
copies itself to `%TEMP%` and runs the copy, which Smart App Control blocks.

Releases aren't code-signed yet, so SmartScreen may warn about them and Smart App Control may
block them. Signing would slot into the workflow's Build step once there's a certificate.
