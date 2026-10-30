# Security

If you find a security problem in any of Mullion's apps or its installer, please report it
privately rather than in a public issue: on GitHub, open the **Security** tab and choose
**Report a vulnerability**. You'll get a reply as soon as possible, and credit in the fix's
release notes if you'd like it.

Useful things to include: which app, what an attacker could do, the steps to reproduce it, and the
app's and Windows' versions you tried.

## What each app does and doesn't do

Each app lists exactly what it reads, keeps and can do in its own SECURITY.md:

- [Oriel](apps/oriel/SECURITY.md)
- [Lintel](apps/lintel/SECURITY.md)
- [Fanlight](apps/fanlight/SECURITY.md)
- [Bay](apps/bay/SECURITY.md)
- [Sash](apps/sash/SECURITY.md)

The apps share code (`core/`), not data: each keeps its own files in its own folder under
`%APPDATA%`, and none of them reads another's.

## Mullion itself

`Mullion.exe` has the suite's tray icon, sign-in entry and settings window. It:

- starts each app that's on, from its own folder or where the app's own installer put it, and
  asks a running app to show itself, open its settings or quit, by starting its exe again with
  `--mullion` and `--show`, `--settings` or `--quit` (`core/src/suite.rs`). It can't start any
  other program.
- reads which of the apps are running, by their exe's path, while its settings window is open.
- turns off an app's own sign-in entry when it starts it, so the app isn't started twice.
- keeps one file, `%APPDATA%\Mullion\settings.json`: the apps you turned off.
- opens the release page on GitHub in your browser if you click **Get it** for an app that isn't
  installed.
- checks for updates, unless you turn **Check for updates** off: once a day (the first a minute
  after it starts, and never before you've pressed Start in the welcome), and when you click
  **Check now**, it asks GitHub's API for the suite's latest release
  (`api.github.com/repos/Aduneer/Mullion/releases/latest`), with `Mullion/<version>` as the
  User-Agent. Nothing else is sent. If you click **Update**, it downloads that release's
  `Mullion-<version>-setup.exe` and `SHA256SUMS.txt` from the release on GitHub (no other
  address), checks the installer's SHA-256 against that list, saves it in `%TEMP%\Mullion` and
  runs it silently, which quits Mullion and starts it again. A copy not installed by Mullion's
  installer only opens the release page (`apps/mullion/src-tauri/src/updates.rs`). That's the
  only time Mullion goes online. Like any HTTPS download, it trusts GitHub and the release:
  the checksum catches a broken download, not a release someone else published from the
  repo.

## Mullion's installer

`Mullion-<version>-setup.exe` installs for you only, with no admin rights, into
`%LOCALAPPDATA%\Mullion`: `mullion.exe`, the exe of each app you tick, and an uninstaller, with
one Start menu entry, one entry in Installed apps and, if you leave it ticked, one sign-in entry.
If an app you tick is also installed on its own, it asks before running that copy's uninstaller.
Its uninstaller removes all of that, and the apps' settings only if you tick that box and the app
isn't still installed on its own.
