# Security

If you find a security problem in Apron, please report it privately rather than in a public issue:
on GitHub, open the **Security** tab and choose **Report a vulnerability**. You'll get a reply as
soon as possible, and credit in the fix's release notes if you'd like it.

Useful things to include: what an attacker could do, the steps to reproduce it, and the Apron and
Windows versions you tried.

## What Apron does and doesn't do

Two things Apron does are bigger than most apps': **it hides Windows' taskbar**, and **it hosts
the notification area**, so the icons other apps put there are sent to Apron. Both are below in
full.

- It never connects to the internet. There's no update check, account or telemetry, and the
  content security policy keeps its pages from loading anything that isn't part of the app.
  **Get it**, shown only when Fanlight isn't installed, opens Mullion's latest release on GitHub
  in your browser, and its button says so.
- **The taskbar.** Unless **Float and auto-hide** is on, Apron hides Windows' taskbars
  (`ShowWindow` on Explorer's `Shell_TrayWnd` and `Shell_SecondaryTrayWnd`), sets the taskbar to
  auto-hide (`SHAppBarMessage`, `ABM_SETSTATE`), and keeps its screen's work area clear of
  the dock (`SystemParametersInfo`, `SPI_SETWORKAREA`, not saved past sign-out). It saves the
  taskbar's auto-hide state as it found it in `%APPDATA%\Apron\taskbar-state`, and puts it all
  back when it quits. `apron-guard.exe`, which Apron starts beside it, waits for Apron to end and, if
  that file is still there, runs `apron.exe --restore-taskbar` to put the taskbar back; Apron's and
  Mullion's uninstallers run the same. It changes nothing else of Explorer's.
- **The notification area.** Apron makes a window of the taskbar's class (`Shell_TrayWnd`), so the
  messages apps send to add, change and remove their icons there come to Apron. It keeps, in memory
  only, what each one carries: its icon, its tooltip, the window and message it answers to, and
  the program it belongs to (by its process, for its file name). It passes every one of them on
  to Explorer's own taskbar unchanged, so Explorer keeps its icons too. Balloon notifications are
  passed on and not shown. A click on an icon is sent only to that icon's own window, as the
  taskbar sends it; Apron lets that program bring a window to the front for the click
  (`AllowSetForegroundWindow`). It reads which icons Windows keeps out of sight from
  `HKEY_CURRENT_USER\Control Panel\NotifyIconSettings`, and changes nothing there. When the
  pointer is on an icon, that program is told, as the taskbar tells it, so it can show its
  tooltip. When a program asks where its icon is (to put its menu beside it), Apron answers with
  where you last clicked or pointed at it.
- **Lintel.** Apron passes the icons on to Lintel's bar, over a named pipe on this PC
  (`\\.\pipe\Mullion.Apron.tray.<session>`, `core/src/tray.rs`), never over the network: the pipe
  refuses remote clients, only your own user account may open it, and Apron hangs up on any
  process that isn't `lintel.exe`. It sends each icon's picture, tooltip, program file name and
  process id, and whether Windows keeps it hidden. It takes back only whether Lintel shows them,
  and clicks and the pointer on an icon, which it passes on exactly as for the dock's own; the
  page can't make it do anything else.
- **Your apps.** To show the running apps, Apron reads the windows Alt+Tab would list (their
  titles, whether minimized or in front) and the path of the program each belongs to, kept in
  memory only. It hears windows come and go through WinEvent hooks (no keyboard or mouse hook).
  What it can do to an app is what you click: bring one of its windows to the front, minimize it,
  ask its windows to close (**Quit** asks, as the × button does, and never ends a process), or
  open it or a new window of it (`ShellExecute` of the program or the shortcut you pinned).
- What it reads once: the first time it starts, the shortcuts pinned to the taskbar
  (`%APPDATA%\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar`), for their targets.
  When you choose **Add…** in its settings, the Start menu's shortcuts (yours and everyone's), for
  their names, targets and icons. A file you drop on the dock is only pinned, never opened.
- Whether an app runs as administrator (its process's token, read-only), to say why Apron can't
  reach it.
- With **Glass** on, the wallpaper's file, to paint a blurred copy of it behind the dock.
- It starts Fanlight from Mullion's folder or where its own installer put it, with `--toggle
  --apps`, the same way Mullion starts it; besides that and `apron-guard.exe`, it starts no program
  of its own accord.
- Floating, it looks where the pointer is ten times a second, to show the dock at the edge. It
  never reads keys or mouse buttons.
- What it keeps, in `%APPDATA%\Apron`: `settings.json`, your settings and the apps you pinned (their
  paths and names), and `taskbar-state` while it's hiding the taskbar. Nothing else.
- It runs as you, without admin rights, and installs per user. **Start at sign-in** adds an entry
  under `HKEY_CURRENT_USER\...\Run`, which uninstalling removes.
- Like any app run from a folder you can write to, the portable exe loads some Windows DLLs by
  name, so a malicious DLL dropped next to it could be loaded. Keep the portable copy in a folder
  of its own, not in Downloads.
- Releases are built by GitHub Actions from the tagged commit. Each release lists the SHA-256 of
  its files in `SHA256SUMS.txt`. They aren't code-signed yet.
