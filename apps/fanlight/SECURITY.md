# Security

If you find a security problem in Fanlight, please report it privately rather than in a public
issue: on GitHub, open the **Security** tab and choose **Report a vulnerability**. You'll get a
reply as soon as possible, and credit in the fix's release notes if you'd like it.

Useful things to include: what an attacker could do, the steps to reproduce it, and the Fanlight
and Windows versions you tried.

## What Fanlight does and doesn't do

- It never connects to the internet itself. There's no update check, account or telemetry, and the
  content security policy keeps the page from loading anything that isn't part of the app. A web
  search, or a web address you typed, is handed to your default browser when you pick it, and
  nothing is sent anywhere before that: there are no search suggestions from the web.
- What it reads stays on your PC:
  - the apps the Start menu lists, and their icons;
  - the names, attributes and times of the files and folders in the folders it searches (your
    Desktop, Documents, Downloads, Pictures, Music and Videos, and the ones you add), never what's
    in them;
  - for the preview only, when you ask for it: the start of the selected file (at most 32 KB) and a
    thumbnail from Windows' shell. Files that are only in OneDrive, not on the PC, aren't read, so
    a preview never downloads anything;
  - the titles of the open windows and the programs they belong to;
  - the text you copy, when the clipboard history is on (see below);
  - your number format, from Windows' settings, for sums.
- What it keeps, all in `%APPDATA%\Fanlight`:
  - `settings.json`, your settings;
  - `history.json`, what you've opened from the panel (an app's id, a file's path, a command's
    name) and the searches you opened them from, so they rank higher next time. **Forget** in
    Settings empties it;
  - `clipboard.json`, the clipboard history, only if **Keep it after Fanlight quits** is on.
    Otherwise the history is in memory only, and gone when Fanlight quits.
- The clipboard history leaves out what other apps mark as private, by the same clipboard formats
  Windows' own history honours (`ExcludeClipboardContentFromMonitorProcessing`,
  `CanIncludeInClipboardHistory`, `Clipboard Viewer Ignore`), which is how password managers keep
  passwords out of it. It keeps text only, up to 256 KB a copy. Windows tells Fanlight when the
  clipboard changes; it never polls it.
- The shortcut is registered with `RegisterHotKey`. Fanlight has no keyboard hook, so it never
  sees any other key you press, and it only reads what you type into its own field.
- The page can't run programs of its choosing. Each result it asks to open is checked first
  (`open` in `src-tauri/src/main.rs`):
  - an app must be one the Start menu lists; **Run as administrator** asks Windows, which shows
    its own prompt;
  - a file or folder must exist, and opens as a double-click in File Explorer would, or is shown
    in its folder;
  - a Settings page or a command must be one of Fanlight's own (`src-tauri/src/catalog.rs`);
  - a window must still belong to the process it did when it was listed, since Windows reuses
    window handles. Fanlight only brings it to the front.
- **Paste** puts the entry on the clipboard, gives the focus back to the window you were in and
  presses <kbd>Ctrl</kbd> + <kbd>V</kbd> there with `SendInput`. Show desktop, Take a screenshot
  and Emoji press Windows' own shortcuts for them (<kbd>Win</kbd> with D, Shift+S or the period
  key) the same way.
- It can lock the PC, put it to sleep, sign out, restart or shut down (`src-tauri/src/power.rs`).
  Sign out, restart and shut down ask for a second Enter first, and none of them force apps to
  close.
- It runs as you, without admin rights, and installs per user.
- Like any app run from a folder you can write to, the portable exe loads some Windows DLLs
  by name, so a malicious DLL dropped next to it could be loaded. Keep the portable copy in a
  folder of its own, not in Downloads.
- Releases are built by GitHub Actions from the tagged commit. Each release lists the SHA-256 of
  its files in `SHA256SUMS.txt`. They aren't code-signed yet.
