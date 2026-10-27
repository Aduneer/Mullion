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
  installed. It never goes online itself.

## Mullion's installer

`Mullion-<version>-setup.exe` installs for you only, with no admin rights, into
`%LOCALAPPDATA%\Mullion`: `mullion.exe`, the exe of each app you tick, and an uninstaller, with
one Start menu entry, one entry in Installed apps and, if you leave it ticked, one sign-in entry.
If an app you tick is also installed on its own, it asks before running that copy's uninstaller.
Its uninstaller removes all of that, and the apps' settings only if you tick that box and the app
isn't still installed on its own.
