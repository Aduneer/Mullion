# Security

If you find a security problem in Bay, please report it privately rather than in a public issue:
on GitHub, open the **Security** tab and choose **Report a vulnerability**. You'll get a reply as
soon as possible, and credit in the fix's release notes if you'd like it.

Useful things to include: what an attacker could do, the steps to reproduce it, and the Bay and
Windows versions you tried.

## What Bay does and doesn't do

- It never connects to the internet. There's no update check, account or telemetry, and the
  content security policy keeps its pages from loading anything that isn't part of the app, apart
  from the wallpaper Bay itself serves them.
- What it reads stays on your PC, and only while the overview opens:
  - the list of open windows Alt+Tab would show: each one's title, which program it belongs to,
    where it is and whether it's minimized;
  - each of those programs' path, for its icon, and the icon, from Windows' shell;
  - a live picture of each window and of the desktop, from Windows' compositor (DWM). The
    pictures go straight from DWM to the screen: Bay never sees their pixels, and never saves or
    sends them;
  - your wallpaper: which picture is on each screen and how it's fitted, from Windows, and the
    picture file itself, which it shows blurred behind the windows. The pages can ask only for the
    file Windows names for one of your screens.
- What it keeps, in `%APPDATA%\Bay`: `settings.json`, your settings. Nothing else.
- What it can do with your windows:
  - bring one you pick to the front (and restore it if it's minimized), and move the pointer into
    its middle. A window must still belong to the process it did when the overview opened, since
    Windows reuses window handles;
  - move one you drag onto another screen there, the way dragging its title bar would, making it
    smaller only if it's bigger than that screen. The same check applies. Windows doesn't let it
    move the windows of programs running as administrator; such a window stays where it was;
  - nothing else: it never closes a window or types into one.
- The **Desktops** button presses <kbd>Win</kbd> + <kbd>Tab</kbd> with `SendInput` once the
  overview has closed, to open Windows' Task View.
- The shortcut is registered with `RegisterHotKey`. Bay has no keyboard or mouse hook, so it never
  sees any other key you press. While you drag a window it reads the pointer's position and
  whether the button is still down, and stops when you let go.
- It runs as you, without admin rights, and installs per user. **Start at sign-in** adds an entry
  under `HKEY_CURRENT_USER\...\Run`, which uninstalling removes.
- Like any app run from a folder you can write to, the portable exe loads some Windows DLLs by
  name, so a malicious DLL dropped next to it could be loaded. Keep the portable copy in a folder
  of its own, not in Downloads.
- Releases are built by GitHub Actions from the tagged commit. Each release lists the SHA-256 of
  its files in `SHA256SUMS.txt`. They aren't code-signed yet.
