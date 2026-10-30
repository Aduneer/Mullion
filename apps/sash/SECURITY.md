# Security

If you find a security problem in Sash, please report it privately rather than in a public issue:
on GitHub, open the **Security** tab and choose **Report a vulnerability**. You'll get a reply as
soon as possible, and credit in the fix's release notes if you'd like it.

Useful things to include: what an attacker could do, the steps to reproduce it, and the Sash and
Windows versions you tried.

## What Sash does and doesn't do

- It never connects to the internet. There's no update check, account or telemetry, and the
  content security policy keeps its pages from loading anything that isn't part of the app.
- What it reads stays on your PC, and only when you press one of its shortcuts:
  - which window is in front, and whether Alt+Tab would list it: its title, which program it
    belongs to, whether it's minimized, maximized or not responding;
  - where that window is (its rectangle, and what's seen of it from Windows' compositor, DWM);
  - your screens: where each is, its work area (without the taskbar and app bars) and its
    scaling.
- What it keeps, in `%APPDATA%\Sash`: `settings.json`, your settings. Nothing else. Where a
  window was before Sash moved it (for Restore) is kept in memory only, until Sash quits.
- What it can do with your windows: only the window in front, only when you press a shortcut,
  and only one Alt+Tab would list (never the desktop, the taskbar or a menu):
  - move and size it, maximize it, or restore it from maximized, with `SetWindowPos` and
    `ShowWindow`, as dragging its edges or clicking its buttons would;
  - nothing else: it never brings a window forward, closes one, or types into one.
- Windows doesn't let it move the windows of programs running as administrator. Sash learns that
  by asking Windows to redraw the window's frame where it is and seeing it refused; it never
  looks into other programs. It then shows a note saying so, which takes no focus and closes
  itself.
- The shortcuts are registered with `RegisterHotKey`. Sash has no keyboard or mouse hook, so it
  never sees any other key you press, and never reads the pointer. While you choose a new
  shortcut, its settings page reads the keys you press in that window only, and Sash lets go of
  its own shortcuts until you've chosen.
- **Open its settings**, beside *Windows' own Snap*, opens Windows' Settings at
  `ms-settings:multitasking`. Sash never changes Windows' settings itself.
- It runs as you, without admin rights, and installs per user. **Start at sign-in** adds an entry
  under `HKEY_CURRENT_USER\...\Run`, which uninstalling removes.
- Like any app run from a folder you can write to, the portable exe loads some Windows DLLs by
  name, so a malicious DLL dropped next to it could be loaded. Keep the portable copy in a folder
  of its own, not in Downloads.
- Releases are built by GitHub Actions from the tagged commit. Each release lists the SHA-256 of
  its files in `SHA256SUMS.txt`. They aren't code-signed yet.
