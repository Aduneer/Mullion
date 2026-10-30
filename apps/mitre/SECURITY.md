# Security

If you find a security problem in Mitre, please report it privately rather than in a public issue:
on GitHub, open the **Security** tab and choose **Report a vulnerability**. You'll get a reply as
soon as possible, and credit in the fix's release notes if you'd like it.

Useful things to include: what an attacker could do, the steps to reproduce it, and the Mitre and
Windows versions you tried.

## What Mitre does and doesn't do

- It never connects to the internet. There's no update check, account or telemetry, and the
  content security policy keeps its pages from loading anything that isn't part of the app.
  **Get it**, shown only when Bay or Fanlight isn't installed, opens Mullion's latest release on
  GitHub in your browser, and its button says so.
- How it hears the pointer: it registers for Raw Input from the mouse
  (`RegisterRawInputDevices`), so Windows tells it when the mouse moves, and then asks where the
  pointer is (`GetCursorPos`). It never reads the raw input itself (how far the mouse moved, its
  buttons), and has no mouse or keyboard hook. Once a second it also asks where the pointer is, in
  case Windows stopped telling it.
- What else it reads, only when the pointer reaches a corner, and kept in memory only:
  - whether a mouse button is held (`GetAsyncKeyState`, for the five mouse buttons only);
  - the window in front: its rectangle, whether it has a title bar, which screen it's on and
    which program it belongs to (so Explorer's and Mullion's own windows, like Bay's overview,
    never count as full screen);
  - whether Windows says a full-screen app, game or presentation is running
    (`SHQueryUserNotificationState`);
  - your screens: where each is, its scaling, and its monitor's device name, to keep each
    screen's corners apart.
- What it keeps, in `%APPDATA%\Mitre`: `settings.json`, your settings, with each screen's corners
  under its monitor's device name. Nothing else.
- What a corner can do, and only the one you chose for it:
  - start Bay or Fanlight from Mullion's folder or where their own installers put them, with
    `--toggle` (and Fanlight's `--apps`), the same way Mullion starts them; it starts no other
    program;
  - press <kbd>Win</kbd> + <kbd>D</kbd> or <kbd>Win</kbd> + <kbd>Tab</kbd> (`SendInput`), for the
    desktop and Task View;
  - lock the PC (`LockWorkStation`), or turn the screens off (`SC_MONITORPOWER`, asked of its own
    window).
- The glow is a window of its own that clicks pass through, never takes the focus, and shows
  only in the corner while the pointer waits there.
- It runs as you, without admin rights, and installs per user. **Start at sign-in** adds an entry
  under `HKEY_CURRENT_USER\...\Run`, which uninstalling removes.
- Like any app run from a folder you can write to, the portable exe loads some Windows DLLs by
  name, so a malicious DLL dropped next to it could be loaded. Keep the portable copy in a folder
  of its own, not in Downloads.
- Releases are built by GitHub Actions from the tagged commit. Each release lists the SHA-256 of
  its files in `SHA256SUMS.txt`. They aren't code-signed yet.
