# Security

If you find a security problem in Lintel, please report it privately rather than in a public
issue: on GitHub, open the **Security** tab and choose **Report a vulnerability**. You'll get a
reply as soon as possible, and credit in the fix's release notes if you'd like it.

Useful things to include: what an attacker could do, the steps to reproduce it, and the Lintel
and Windows versions you tried.

## What Lintel does and doesn't do

- It never connects to the internet. There's no update check, account or telemetry, and the
  content security policy keeps the pages from loading anything that isn't part of the app.
- What it reads stays on your PC: the name and windows of the app in front, what's playing and its
  cover, volumes, CPU, memory and network counters, the battery, the name of the network you're
  on, and your clock and calendar format from Windows' settings.
- The pages can't run programs. They can ask Windows to open Task Manager, File Explorer, Oriel
  (only the exe Oriel's own installer registered), the Wi-Fi list and `ms-settings:` pages, and
  nothing else (`run_action` in `src-tauri/src/main.rs`).
- It can bring a window of the app in front to the front, minimize it, or ask it to close, as
  the app menu's items do (`window_action`). **Quit** asks each of that app's windows to close,
  the way clicking their × would; the app can still ask to save your work. Lintel never ends a
  process, and checks a window still belongs to the process you saw before it acts, since
  Windows reuses window handles.
- It can play, pause or skip the current track, like a keyboard's media keys, and change the
  volume of your output and of the apps playing through it.
- It can lock the PC, put it to sleep, sign out, restart or shut down (`src-tauri/src/power.rs`).
  Sign out, restart and shut down ask first, and none of them force apps to close.
- It runs as you, without admin rights, and installs per user. Its settings are in
  `%APPDATA%\Lintel\settings.json`.
- Like any app run from a folder you can write to, the portable exe loads some Windows DLLs
  by name, so a malicious DLL dropped next to it could be loaded. Keep the portable copy in a
  folder of its own, not in Downloads.
- Releases are built by GitHub Actions from the tagged commit. Each release lists the SHA-256 of
  its files in `SHA256SUMS.txt`. They aren't code-signed yet.
