// The preview's stand-in for the Rust backend: made-up but plausible apps, files, windows and
// clipboard entries, searched roughly as search.rs does, and the panel's window (fit, hide)
// done with an iframe over a stand-in desktop.
//
// URL options, all optional:
//   theme=dark|light  palette=nord  glass=on
//   q=lisbon (typed in)  only=apps|files|actions|clipboard  sel=2 (↓ that many times)  preview=1 (Space)
//   view=settings  apps=list (the Apps category as a list, not a grid)
//   demo=1 (types a tour of it, for the README's animation)
"use strict";
(() => {
  const q = new URLSearchParams(location.search);
  const $ = id => document.getElementById(id);
  const win = $("win"), frame = $("panel");
  const log = msg => { $("log").textContent = msg; clearTimeout(log.t); log.t = setTimeout(() => ($("log").textContent = ""), 4000); };

  // A fixed "now", so "Copied 4 min ago" and the dates in the preview are the same every time.
  const NOW = Date.parse("2026-10-24T17:42:00") / 1000;
  const HOME = String.raw`C:\Users\you`;

  let settings = {
    theme: q.get("theme") || "dark", palette: q.get("palette") || "oriel", glass: q.get("glass") === "on", hotkey: "alt+space", engine: "google",
    files: true, folders: [], windows: true, apps_view: q.get("apps") || "grid", clipboard: true, clipboard_size: 100, keep_clipboard: false,
  };
  let autostart = true;

  // --- made-up pictures ------------------------------------------------------------------
  const data = svg => "data:image/svg+xml;base64," + btoa(svg);
  // Plain made-up app icons (no real apps' logos): a rounded square and a glyph.
  const appIcon = (from, to, glyph) => data(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs><rect x="4" y="4" width="56" height="56" rx="14" fill="url(#g)"/><g fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">${glyph}</g></svg>`);
  const fileIcon = (color, label) => data(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M14 4h26l12 12v44H14z" fill="#f4f4f2" stroke="#c9c9c4" stroke-width="2" stroke-linejoin="round"/><path d="M40 4v12h12" fill="#e2e2dd" stroke="#c9c9c4" stroke-width="2" stroke-linejoin="round"/><rect x="8" y="34" width="38" height="16" rx="3" fill="${color}"/><text x="27" y="46" font-family="Segoe UI, sans-serif" font-size="11" font-weight="700" fill="#fff" text-anchor="middle">${label}</text></svg>`);
  const folderIcon = data(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M6 16a4 4 0 0 1 4-4h14l5 5h25a4 4 0 0 1 4 4v29a4 4 0 0 1-4 4H10a4 4 0 0 1-4-4z" fill="#e8a92c"/><path d="M6 24h52v26a4 4 0 0 1-4 4H10a4 4 0 0 1-4-4z" fill="#ffc94d"/></svg>`);
  const PHOTOS = {
    tram: data(`<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300" viewBox="0 0 400 300"><defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#8fd3ff"/><stop offset="1" stop-color="#e8f6ff"/></linearGradient></defs><rect width="400" height="300" fill="url(#s)"/><path d="M0 120h70v180H0zM320 90h80v210h-80z" fill="#f2d7b6"/><path d="M0 120h70v12H0zM320 90h80v12h-80z" fill="#c9774a"/><g fill="#5b88b5"><rect x="12" y="150" width="16" height="24"/><rect x="40" y="150" width="16" height="24"/><rect x="335" y="125" width="16" height="24"/><rect x="365" y="125" width="16" height="24"/><rect x="335" y="175" width="16" height="24"/><rect x="365" y="175" width="16" height="24"/></g><path d="M0 262h400v38H0z" fill="#8c8a86"/><path d="M0 270h400M0 284h400" stroke="#5f5d59" stroke-width="3"/><path d="M200 40v58" stroke="#333" stroke-width="3"/><rect x="92" y="98" width="216" height="150" rx="18" fill="#ffd21f"/><rect x="92" y="186" width="216" height="62" rx="10" fill="#f5f2ea"/><g fill="#36506b"><rect x="108" y="116" width="40" height="46" rx="5"/><rect x="158" y="116" width="40" height="46" rx="5"/><rect x="208" y="116" width="40" height="46" rx="5"/><rect x="258" y="116" width="34" height="46" rx="5"/></g><text x="200" y="218" font-family="Segoe UI, sans-serif" font-size="22" font-weight="700" fill="#333" text-anchor="middle">28</text><circle cx="130" cy="254" r="12" fill="#333"/><circle cx="270" cy="254" r="12" fill="#333"/></svg>`),
    sunset: data(`<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300" viewBox="0 0 400 300"><defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3b2a6b"/><stop offset=".55" stop-color="#f0766b"/><stop offset="1" stop-color="#ffc76e"/></linearGradient></defs><rect width="400" height="300" fill="url(#s)"/><circle cx="270" cy="180" r="46" fill="#ffe3a1"/><path d="M0 200h400v100H0z" fill="#2b5c8a" opacity=".85"/><path d="M220 210h100M240 224h60M255 238h30" stroke="#ffe3a1" stroke-width="4" stroke-linecap="round" opacity=".7"/><path d="M0 170l30-20h20v-24h22v24l20 14h26v-34l18-14 18 14v50l24-12h30v74H0z" fill="#241a3a"/><path d="M300 200l20-30 60 4 20 26z" fill="#241a3a"/></svg>`),
  };
  const ICONS = {
    code: ["#7a8cff", "#4a5ae0", `<path d="M26 22l-10 10 10 10M38 22l10 10-10 10"/>`],
    calc: ["#6c7380", "#3f4550", `<rect x="20" y="16" width="24" height="32" rx="4"/><path d="M26 25h12M27 34h2M35 34h2M27 41h2M35 41h2"/>`],
    calendar: ["#5aa0f2", "#2f6fd1", `<rect x="18" y="20" width="28" height="26" rx="4"/><path d="M18 28h28M26 16v8M38 16v8"/>`],
    camera: ["#4a4f5c", "#22252c", `<rect x="16" y="22" width="32" height="22" rx="5"/><circle cx="32" cy="33" r="6"/>`],
    notes: ["#ffd36b", "#f0a92e", `<path d="M22 18h20v28H22zM27 26h10M27 32h10M27 38h6"/>`],
    paint: ["#ff8c6b", "#e0577a", `<path d="M20 44c4 0 6-3 6-6s4-4 6-2-1 8-12 8zM30 34l14-16 3 3-14 16"/>`],
    photos: ["#4fd1c5", "#2b8fa8", `<rect x="16" y="18" width="32" height="28" rx="4"/><path d="M18 42l9-10 7 7 4-4 8 7"/><circle cx="38" cy="26" r="3"/>`],
    terminal: ["#4a4f5c", "#22252c", `<path d="M19 24l8 8-8 8M31 41h14"/>`],
    music: ["#4fd18a", "#1f9d57", `<path d="M26 40V22l16-4v18"/><circle cx="23" cy="40" r="3"/><circle cx="39" cy="36" r="3"/>`],
    explorer: ["#ffd36b", "#f0a92e", `<path d="M16 22h12l4 4h16v16H16z"/>`],
    browser: ["#5aa0f2", "#2f6fd1", `<circle cx="32" cy="32" r="15"/><path d="M17 32h30M32 17c-6 8-6 22 0 30M32 17c6 8 6 22 0 30"/>`],
    mail: ["#3d8bfd", "#1d5fd0", `<rect x="16" y="20" width="32" height="24" rx="3"/><path d="M16 22l16 12 16-12"/>`],
    chat: ["#9a7cff", "#6a4ee0", `<path d="M18 20h28v18H30l-8 7v-7h-4z"/>`],
    design: ["#ff7ab0", "#c0407e", `<circle cx="26" cy="26" r="7"/><rect x="31" y="31" width="14" height="14" rx="3"/>`],
    sheet: ["#3fb37f", "#1e7d55", `<rect x="18" y="18" width="28" height="28" rx="3"/><path d="M18 28h28M18 37h28M28 18v28"/>`],
    doc: ["#4f86f7", "#2d5fd0", `<path d="M22 18h20v28H22zM27 26h10M27 32h10M27 38h10"/>`],
    snip: ["#ff8c6b", "#d0456a", `<circle cx="24" cy="40" r="5"/><circle cx="40" cy="40" r="5"/><path d="M27 36l15-20M37 36L22 16"/>`],
    store: ["#5aa0f2", "#3a5fd9", `<path d="M18 24h28l-3 20H21zM26 24v-3a6 6 0 0 1 12 0v3"/>`],
    task: ["#6fd18a", "#2f9d67", `<path d="M16 36h8l4-10 6 18 4-8h10"/>`],
  };
  // name, id, icon, Store app (an AUMID) or a program
  const APPS = [
    ["Visual Studio Code", String.raw`{6D809377-6AF0-444B-8957-A3773F02200E}\Microsoft VS Code\Code.exe`, "code"],
    ["Calculator", "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", "calc"],
    ["Calendar", "Microsoft.OutlookForWindows_8wekyb3d8bbwe!Calendar", "calendar"],
    ["Camera", "Microsoft.WindowsCamera_8wekyb3d8bbwe!App", "camera"],
    ["Notepad", "Microsoft.WindowsNotepad_8wekyb3d8bbwe!App", "notes"],
    ["Paint", "Microsoft.Paint_8wekyb3d8bbwe!App", "paint"],
    ["Photos", "Microsoft.Windows.Photos_8wekyb3d8bbwe!App", "photos"],
    ["Terminal", "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App", "terminal"],
    ["Spotify", "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify", "music"],
    ["File Explorer", "Microsoft.Windows.Explorer", "explorer"],
    ["Firefox", String.raw`{6D809377-6AF0-444B-8957-A3773F02200E}\Mozilla Firefox\firefox.exe`, "browser"],
    ["Outlook", "Microsoft.OutlookForWindows_8wekyb3d8bbwe!Microsoft.OutlookforWindows", "mail"],
    ["Discord", String.raw`C:\Users\you\AppData\Local\Discord\Update.exe`, "chat"],
    ["Figma", String.raw`C:\Users\you\AppData\Local\Figma\Figma.exe`, "design"],
    ["Excel", String.raw`{6D809377-6AF0-444B-8957-A3773F02200E}\Microsoft Office\root\Office16\EXCEL.EXE`, "sheet"],
    ["Word", String.raw`{6D809377-6AF0-444B-8957-A3773F02200E}\Microsoft Office\root\Office16\WINWORD.EXE`, "doc"],
    ["Snipping Tool", "Microsoft.ScreenSketch_8wekyb3d8bbwe!App", "snip"],
    ["Microsoft Store", "Microsoft.WindowsStore_8wekyb3d8bbwe!App", "store"],
    ["Task Manager", String.raw`{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\Taskmgr.exe`, "task"],
  ].map(([name, id, icon]) => ({ name, id, icon }));

  const KB = 1024, MB = KB * KB;
  const day = n => NOW - n * 86400;
  const NOTES = `# A weekend in Lisbon

## Friday
- Land 10:40, metro to Baixa-Chiado
- Check in: Rua Augusta 24
- Tram 28 up to Alfama, before the queues
- Dinner: the fish place at Cais do Sodré

## Saturday
- Belém in the morning (pastéis!)
- LX Factory after lunch
- Sunset from the Miradouro da Senhora do Monte

## Sunday
- Flea market, Feira da Ladra (Tue and Sat only, check)
- Train to Sintra if the weather holds
- Back by 18:00 for the flight

## Packing
- Comfy shoes: it's all hills
- Adapter, sunglasses, a light jacket
`;
  // The files and folders in the index: path, size, modified and created (days ago), and what a
  // preview shows: a picture, text, or a folder's items.
  const FILES = [
    { path: String.raw`C:\Users\you\Pictures\Lisbon 2026`, dir: true, modified: day(2), created: day(9), items: ["Lisbon tram.jpg", "Lisbon sunset.jpg", "IMG_2041.jpg", "IMG_2042.jpg", "IMG_2047.jpg", "IMG_2051.jpg", "IMG_2060.jpg", "Lisbon drone.mp4"] },
    { path: String.raw`C:\Users\you\Pictures\Lisbon 2026\Lisbon tram.jpg`, size: 4.2 * MB, modified: day(9), created: day(9), picture: "tram" },
    { path: String.raw`C:\Users\you\Pictures\Lisbon 2026\Lisbon sunset.jpg`, size: 3.7 * MB, modified: day(8), created: day(8), picture: "sunset" },
    { path: String.raw`C:\Users\you\Documents\Travel\Lisbon trip notes.md`, size: 712, modified: day(3), created: day(21), text: NOTES },
    { path: String.raw`C:\Users\you\Documents\Travel\Lisbon itinerary.pdf`, size: 284 * KB, modified: day(12), created: day(12) },
    { path: String.raw`C:\Users\you\Documents\Budget 2026.xlsx`, size: 46 * KB, modified: day(1), created: day(290) },
    { path: String.raw`C:\Users\you\Documents\Travel`, dir: true, modified: day(3), created: day(400), items: ["Lisbon itinerary.pdf", "Lisbon trip notes.md", "Passport scan.pdf", "Rome 2025.md"] },
    { path: String.raw`C:\Users\you\Desktop\Project plan.docx`, size: 88 * KB, modified: day(0.2), created: day(30) },
    { path: String.raw`C:\Users\you\Downloads\invoice-1043.pdf`, size: 121 * KB, modified: day(5), created: day(5) },
    { path: String.raw`C:\Users\you\Documents\Code\fanlight`, dir: true, modified: day(0.1), created: day(14), items: [".github", "assets", "docs", "src-tauri", "tools", "ui", "CONTRIBUTING.md", "LICENSE", "README.md", "SECURITY.md"] },
    { path: String.raw`C:\Users\you\Music\Road trip.m3u`, size: 3 * KB, modified: day(40), created: day(40), text: "#EXTM3U\nM83 - Midnight City.mp3\nPhoenix - 1901.mp3\nTame Impala - The Less I Know the Better.mp3\n" },
  ].map(f => ({ ...f, name: f.path.split("\\").pop() }));
  const isDir = path => FILES.find(f => f.path === path)?.dir;

  const WINDOWS = [
    { hwnd: 4242, pid: 812, title: "Weekend away - Notes", app: "Notepad", path: String.raw`C:\Program Files\WindowsApps\Microsoft.WindowsNotepad\Notepad\Notepad.exe`, icon: "notes" },
    { hwnd: 4243, pid: 990, title: "Midnight City - M83", app: "Spotify", path: String.raw`C:\Users\you\AppData\Roaming\Spotify\Spotify.exe`, icon: "music" },
    { hwnd: 4244, pid: 1404, title: "search.rs - fanlight - Visual Studio Code", app: "Visual Studio Code", path: String.raw`C:\Users\you\AppData\Local\Programs\Microsoft VS Code\Code.exe`, icon: "code" },
    { hwnd: 4245, pid: 1500, title: "Inbox - Outlook", app: "Outlook", path: String.raw`C:\Program Files\WindowsApps\Microsoft.OutlookForWindows\olk.exe`, icon: "mail", minimized: true },
  ];
  let clips = [
    { id: 7, text: "Rua Augusta 24, 1100-053 Lisboa", at: NOW - 4 * 60, app: "Firefox" },
    { id: 6, text: "1Z 999 AA1 01 2345 6784", at: NOW - 38 * 60, app: "Outlook" },
    { id: 5, text: "fn fit(window: WebviewWindow, height: f64, wide: Option<bool>) {\n    let height = height.clamp(FIELD, MAX_HEIGHT).ceil();\n    let width = if wide.unwrap_or(false) { WIDE } else { WIDTH };\n}", at: NOW - 2 * 3600, app: "Visual Studio Code" },
    { id: 4, text: "https://www.visitportugal.com/en/tram-28", at: NOW - 5 * 3600, app: "Firefox" },
    { id: 3, text: "Hi Sam, the flights are sorted: out on Friday at 7:05, home on Sunday at 19:30. Hotel details tonight!", at: NOW - 26 * 3600, app: "Outlook" },
    { id: 2, text: "#3987e5", at: NOW - 3 * 86400, app: "Figma" },
  ];
  const COMMANDS = [
    ["lock", "Lock", "lock screen lock pc away", "Lock the PC"],
    ["sleep", "Sleep", "suspend standby", "Put the PC to sleep"],
    ["signout", "Sign out", "log off logout log out sign off", "Sign out of Windows", true],
    ["restart", "Restart", "reboot restart pc", "Restart the PC", true],
    ["shutdown", "Shut down", "shutdown power off turn off", "Shut the PC down", true],
    ["desktop", "Show desktop", "minimize all hide windows", "As Win+D does"],
    ["screenshot", "Take a screenshot", "snip snipping tool screen capture print screen", "Snip part of the screen, as Win+Shift+S does"],
    ["emoji", "Emoji", "emoji smiley gif symbols kaomoji", "Windows' emoji panel, for the app you were in"],
    ["clipboard", "Clipboard history", "clipboard copied paste history cb", "What you've copied lately"],
    ["clear-clipboard", "Clear clipboard history", "clipboard forget erase", "Forget everything Fanlight kept from the clipboard", true],
    ["settings", "Fanlight Settings", "fanlight preferences options config setup", "Shortcut, look, what to search"],
    ["reindex", "Update file index", "fanlight files index rescan refresh", "Look through your folders again"],
    ["quit", "Quit Fanlight", "fanlight exit close", "Until you start it again"],
  ].map(([id, name, words, detail, confirm]) => ({ id, name, words, detail, confirm: !!confirm }));
  const PAGES = [
    ["Display", "screen resolution scale monitor size", "ms-settings:display"],
    ["Night light", "blue light warm evening", "ms-settings:nightlight"],
    ["Sound", "volume speakers microphone audio output", "ms-settings:sound"],
    ["Bluetooth & devices", "bluetooth pair headphones mouse keyboard", "ms-settings:bluetooth"],
    ["Wi-Fi", "wifi wireless network internet", "ms-settings:network-wifi"],
    ["Background", "wallpaper desktop picture", "ms-settings:personalization-background"],
    ["Colors", "dark mode light mode accent theme", "ms-settings:colors"],
    ["Default apps", "default browser open with", "ms-settings:defaultapps"],
    ["Installed apps", "uninstall programs apps features remove", "ms-settings:appsfeatures"],
    ["Windows Update", "updates patch upgrade", "ms-settings:windowsupdate"],
    ["Power & battery", "battery saver energy sleep power plan", "ms-settings:powersleep"],
    ["Storage", "disk space cleanup drive", "ms-settings:storagesense"],
  ].map(([name, words, uri]) => ({ name, words, uri }));
  const ENGINES = [["google", "Google", "https://www.google.com/search?q="], ["bing", "Bing", "https://www.bing.com/search?q="], ["duckduckgo", "DuckDuckGo", "https://duckduckgo.com/?q="], ["brave", "Brave Search", "https://search.brave.com/search?q="], ["ecosia", "Ecosia", "https://www.ecosia.org/search?q="], ["startpage", "Startpage", "https://www.startpage.com/do/search?q="], ["kagi", "Kagi", "https://kagi.com/search?q="]];

  // --- searching, roughly as matcher.rs and search.rs do --------------------------------------
  // As matcher.rs: every word of the query matches the name, the best way it can.
  function wordStarts(name) {
    const starts = [];
    let prev = null;
    [...name].forEach((c, i) => {
      const an = /[\p{L}\p{N}]/u.test(c), pan = prev != null && /[\p{L}\p{N}]/u.test(prev);
      const alpha = /\p{L}/u.test(c), palpha = prev != null && /\p{L}/u.test(prev);
      if (an && (prev == null || !pan || (prev === prev.toLowerCase() && prev !== prev.toUpperCase() && c !== c.toLowerCase()) || palpha !== alpha)) starts.push(i);
      prev = c;
    });
    return starts;
  }
  function initials(w, lower, starts) {
    const go = (w, from) => {
      if (!w) return true;
      if (from >= starts.length) return false;
      const word = lower.slice(starts[from], starts[from + 1] ?? lower.length);
      let common = 0;
      while (common < word.length && common < w.length && word[common] === w[common]) common++;
      for (let k = common; k >= 1; k--) if (go(w.slice(k), from + 1)) return true;
      return false;
    };
    return starts.some((_, i) => go(w, i));
  }
  function gaps(w, s) {
    let i = 0, runs = 0, last = -2;
    for (const c of w) {
      while (i < s.length && s[i] !== c) i++;
      if (i === s.length) return null;
      if (last + 1 !== i) runs++;
      last = i++;
    }
    return runs - 1;
  }
  function wordScore(w, lower, starts) {
    if (starts.some(i => lower.startsWith(w, i))) return lower.startsWith(w) ? 800 : 700;
    if (w.length >= 2 && initials(w, lower, starts)) return 650;
    if (lower.includes(w)) return 450;
    if (w.length >= 3) { const g = gaps(w, lower); return g == null ? null : Math.max(60, 300 - g * 25); }
    return null;
  }
  function score(query, name) {
    const whole = query.trim().toLowerCase(), words = whole.split(/\s+/).filter(Boolean);
    if (!words.length) return null;
    const lower = name.toLowerCase();
    const shorter = s => s - Math.trunc(Math.min(60, Math.max(0, lower.length - whole.length)) / 3);
    if (lower === whole) return 1000;
    if (lower.startsWith(whole)) return shorter(900);
    const starts = wordStarts(name);
    let total = 0;
    for (const w of words) {
      const s = wordScore(w, lower, starts);
      if (s == null) return null;
      total += s;
    }
    return shorter(Math.trunc(total / words.length));
  }
  // As catalog.rs: by its name, by one of the other words it goes by, or by all of them together.
  const listed = (query, name, words) => {
    const together = score(query, `${name} ${words}`);
    const best = Math.max(score(query, name) ?? -1e9, Math.max(...words.split(" ").map(w => score(query, w) ?? -1e9)) - 120, together >= 400 ? together - 200 : -1e9);
    return best > -1e8 ? best : null;
  };
  const KINDS = [["app", "Applications", 6, 100], ["command", "Commands", 3, 40], ["page", "System Settings", 4, -20], ["window", "Windows", 4, -60], ["clip", "Clipboard", 4, -140], ["folder", "Folders", 3, 10], ["file", "Documents", 8, 0]];
  const CATEGORIES = { apps: ["app", "window"], files: ["file", "folder"], actions: ["command", "page"], clipboard: ["clip"] };
  const tidy = p => p.startsWith(HOME) ? "~" + p.slice(HOME.length) : p;
  const oneLine = (t, max) => { const s = t.split(/\s+/).filter(Boolean).join(" "); return s.length > max ? s.slice(0, max) + "…" : s; };
  const ago = s => s < 60 ? "just now" : s < 3600 ? `${Math.floor(s / 60)} min ago` : s < 86400 ? `${Math.floor(s / 3600)} h ago` : `${Math.floor(s / 86400)} days ago`;
  const hit = (key, kind, title, subtitle, icon, score, confirm) => ({ key, kind, title, subtitle, ...(icon ? { icon } : {}), ...(confirm ? { confirm } : {}), score });
  const clipHit = (c, s) => hit(`clip:${c.id}`, "clip", oneLine(c.text, 200), c.app ? `Copied from ${c.app} ${ago(NOW - c.at)}` : `Copied ${ago(NOW - c.at)}`, null, s);
  const isPicture = p => /\.(jpe?g|png|gif|bmp|webp|heic|tiff?|mp4|mov|mkv|avi|wmv)$/i.test(p);

  const UNITS = { km: ["kilometer", "kilometers", 1000], mi: ["mile", "miles", 1609.344], m: ["meter", "meters", 1], ft: ["foot", "feet", 0.3048], kg: ["kilogram", "kilograms", 1], lb: ["pound", "pounds", 0.45359237] };
  const UNIT_ALIASES = { kilometer: "km", kilometers: "km", mile: "mi", miles: "mi", meter: "m", meters: "m", feet: "ft", foot: "ft", lbs: "lb", pounds: "lb", pound: "lb" };
  const fmt = v => String(Number(v.toPrecision(10)));
  function calc(text) {
    const u = text.trim().toLowerCase().match(/^([\d.]+)\s*([a-z]+)\s+(?:in|to|as)\s+([a-z]+)$/);
    if (u) {
      const [from, to] = [UNIT_ALIASES[u[2]] ?? u[2], UNIT_ALIASES[u[3]] ?? u[3]];
      if (!UNITS[from] || !UNITS[to] || (["kg", "lb"].includes(from) !== ["kg", "lb"].includes(to))) return null;
      const v = Number(u[1]), r = v * UNITS[from][2] / UNITS[to][2];
      return { value: fmt(r), detail: `${fmt(v)} ${UNITS[from][v === 1 ? 0 : 1]} in ${UNITS[to][1]}` };
    }
    const t = text.replace(/\s+/g, "");
    if (!/\d/.test(t) || !/[+\-*/^×÷x()]/.test(t.replace(/^-/, "")) || !/^[\d.+\-*/^×÷x()]+$/.test(t)) return null;
    try {
      const v = Function(`"use strict";return (${t.replace(/[×x]/g, "*").replace(/÷/g, "/").replace(/\^/g, "**")})`)();
      return Number.isFinite(v) ? { value: fmt(v), detail: text.trim() } : null;
    } catch { return null; }
  }

  function search(text, only) {
    text = text.trim();
    const kinds = only && CATEGORIES[only];
    const wants = k => !kinds || kinds.includes(k);
    if (!text && !kinds) return { sections: [] };
    const lower = text.toLowerCase();
    const cb = ["cb", "clipboard", "clip"].includes(lower) ? "" : ["cb ", "clipboard ", "clip "].map(p => lower.startsWith(p) ? text.slice(p.length).trim() : null).find(r => r != null);
    if (!kinds && cb != null) {
      const hits = clips.flatMap(c => { const s = cb ? score(cb, c.text) : 0; return s == null ? [] : [clipHit(c, s)]; });
      return { sections: [{ title: clips.length ? "Clipboard" : "Clipboard: nothing copied yet", hits }] };
    }
    const all = !text;
    const floor = text.length < 3 ? 600 : 0;
    const ok = s => all ? 0 : (s != null && s >= floor ? s : null);
    let hits = [];
    if (wants("app")) for (const a of APPS) { const s = ok(score(text, a.name)); if (s != null) hits.push(hit(`app:${a.id}`, "app", a.name, "Application", `app:${a.id}`, s)); }
    if (wants("page")) for (const p of PAGES) { const s = ok(listed(text, p.name, p.words)); if (s != null) hits.push(hit(`page:${p.uri}`, "page", p.name, "Settings", null, s)); }
    if (wants("command")) for (const c of COMMANDS) { const s = ok(listed(text, c.name, c.words)); if (s != null) hits.push(hit(`cmd:${c.id}`, "command", c.name, c.detail, null, s, c.confirm)); }
    if (wants("window")) for (const w of WINDOWS) {
      const s = all ? 0 : ok(Math.max(score(text, w.title) ?? -1e9, (score(text, w.app) ?? -1e9) - 50));
      if (s != null && s > -1e8) hits.push(hit(`win:${w.hwnd}:${w.pid}`, "window", w.title, w.minimized ? `${w.app} · minimized` : w.app, `file:${w.path}`, s));
    }
    if (wants("clip") && (kinds || text.length >= 3)) for (const c of clips) { const s = all ? 0 : score(text, oneLine(c.text, 2000)); if (s != null) hits.push(clipHit(c, s)); }
    if (wants("file") && !all) for (const f of FILES) {
      const s = ok(score(text, f.name));
      if (s == null) continue;
      const kind = f.dir ? "folder" : "file";
      const icon = f.dir ? `folder:${f.path}` : isPicture(f.path) ? `thumb:${f.path}` : `file:${f.path}`;
      hits.push(hit(`${kind}:${f.path}`, kind, f.name, tidy(f.path.slice(0, f.path.lastIndexOf("\\"))), icon, s));
    }
    for (const h of hits) h.score += KINDS.find(k => k[0] === h.kind)[3];
    hits.sort((a, b) => b.score - a.score);

    let top = null, answer;
    if (kinds) { if (!all && hits.length) top = hits.shift(); }
    else if ((answer = calc(text))) top = hit(`calc:${answer.value}`, "calc", answer.value, answer.detail, null, 0);
    else if (hits.length) top = hits.shift();
    const shown = l => kinds ? 60 : l;
    const sections = KINDS.map(([kind, title, limit]) => ({ title, hits: hits.filter(h => h.kind === kind).slice(0, shown(limit)) })).filter(s => s.hits.length);
    const name = ENGINES.find(e => e[0] === settings.engine)[1];
    const webQuery = text.startsWith("?") ? text.slice(1).trim() : text;
    if (webQuery && !kinds) {
      const web = hit(`web:${webQuery}`, "web", `Search ${name} for “${webQuery}”`, "Opens in your browser", null, 0);
      if (text.startsWith("?")) return { top: web, sections: [] };
      sections.push({ title: "Web", hits: [web] });
    }
    return { ...(top ? { top } : {}), sections };
  }

  // --- previews, as preview.rs and main.rs make them -------------------------------------------
  const bytes = n => {
    const units = ["bytes", "KB", "MB", "GB"];
    let v = n, i = 0;
    while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
    return i === 0 ? `${n} bytes` : v < 10 ? `${v.toFixed(1)} ${units[i]}` : `${Math.round(v)} ${units[i]}`;
  };
  const KIND_NAMES = { md: "Markdown document", pdf: "PDF document", xlsx: "Spreadsheet", docx: "Word document", jpg: "Picture", mp4: "Video", m3u: "M3U file" };
  const fact = (label, value) => ({ label, value });
  const time = (label, t) => ({ label, value: "", time: Math.round(t) });
  function preview(key) {
    const [kind, ...restParts] = key.split(":");
    const rest = restParts.join(":");
    if (kind === "file" || kind === "folder") {
      const f = FILES.find(f => f.path === rest);
      if (!f) return null;
      const where = [time("Modified", f.modified), time("Created", f.created), fact("Where", tidy(f.path.slice(0, f.path.lastIndexOf("\\"))))];
      if (f.dir) {
        const items = f.items.map(name => ({ name, dir: !name.includes(".") || name.startsWith(".") })).sort((a, b) => b.dir - a.dir || a.name.localeCompare(b.name));
        return { image: `big:folder:${f.path}`, items, facts: [fact("Kind", "Folder"), fact("Contains", `${f.items.length} items`), ...where] };
      }
      const ext = f.name.split(".").pop().toLowerCase();
      const p = { image: f.picture ? `big:thumb:${f.path}` : `big:file:${f.path}`, facts: [fact("Kind", KIND_NAMES[ext] ?? `${ext.toUpperCase()} file`), fact("Size", bytes(Math.round(f.size))), ...where] };
      if (f.picture) p.picture = true;
      if (f.text) p.text = f.text.trimEnd();
      return p;
    }
    if (kind === "app") {
      const store = rest.includes("!") && !rest.includes("\\");
      const facts = [fact("Kind", store ? "Store app" : "Application")];
      if (!store && rest.includes("\\")) facts.push(fact("Program", rest.split("\\").pop()));
      return { image: `big:app:${rest}`, facts };
    }
    if (kind === "clip") {
      const c = clips.find(c => String(c.id) === rest);
      if (!c) return null;
      const lines = c.text.split("\n").length;
      return { text: c.text, facts: [fact("Kind", "Copied text"), fact("From", c.app), time("Copied", c.at), fact("Length", `${[...c.text].length} characters${lines > 1 ? `, ${lines} lines` : ""}`)] };
    }
    if (kind === "win") {
      const w = WINDOWS.find(w => `${w.hwnd}:${w.pid}` === rest);
      return w && { image: `big:file:${w.path}`, facts: [fact("Kind", "Open window"), fact("App", w.app), fact("Title", w.title), ...(w.minimized ? [fact("State", "Minimized")] : []), fact("Program", w.path)] };
    }
    if (kind === "page") {
      const p = PAGES.find(p => p.uri === rest);
      return p && { facts: [fact("Kind", "Settings page"), fact("Opens", p.uri), fact("Also found by", p.words)] };
    }
    if (kind === "cmd") {
      const c = COMMANDS.find(c => c.id === rest);
      return c && { facts: [fact("Kind", "Command"), fact("Does", c.detail), ...(c.confirm ? [fact("Asks first", "Yes, press Enter twice")] : [])] };
    }
    if (kind === "calc") return { text: rest, facts: [fact("Kind", "Answer"), fact("Enter", "Copies it")] };
    if (kind === "web") {
      const [, name, base] = ENGINES.find(e => e[0] === settings.engine);
      return { facts: [fact("Kind", "Web search"), fact("With", name), fact("Opens", base + encodeURIComponent(rest).replace(/%20/g, "+"))] };
    }
    return null;
  }

  function icon(key) {
    const k = key.replace(/^big:/, "");
    const [kind, ...restParts] = k.split(":");
    const rest = restParts.join(":");
    if (kind === "app") { const a = APPS.find(a => a.id === rest); return a ? appIcon(...ICONS[a.icon]) : null; }
    if (kind === "folder") return folderIcon;
    if (kind === "thumb") return PHOTOS[FILES.find(f => f.path === rest)?.picture] ?? null;
    if (kind === "file") {
      const w = WINDOWS.find(w => w.path === rest);
      if (w) return appIcon(...ICONS[w.icon]);
      if (isDir(rest)) return folderIcon;
      const ext = rest.split(".").pop().toLowerCase();
      const [color, label] = { pdf: ["#d9413a", "PDF"], xlsx: ["#1f8a55", "XLS"], docx: ["#2f6fd1", "DOC"], md: ["#5c6370", "MD"], m3u: ["#9a5cd0", "M3U"], mp4: ["#7a4fd0", "MP4"] }[ext] ?? ["#7c818d", ext.slice(0, 3).toUpperCase()];
      return fileIcon(color, label);
    }
    return null;
  }

  // --- events and commands ----------------------------------------------------------------------
  const listeners = {};
  function listen(event, fn) { (listeners[event] ??= []).push(fn); return () => {}; }
  const emit = (event, payload) => { for (const fn of listeners[event] ?? []) fn(structuredClone(payload)); };

  const commands = {
    hello: () => ({
      settings, hotkey_error: null, hotkeys: ["alt+space", "ctrl+space", "ctrl+alt+space", "alt+shift+space", "win+alt+space", "ctrl+shift+space"],
      engines: ENGINES.map(([id, name]) => [id, name]), autostart, files: 48213, version: "0.1.0",
      settings_path: String.raw`C:\Users\you\AppData\Roaming\Fanlight\settings.json`,
    }),
    search: ({ text, seq, only }) => [seq, search(text, only)],
    icon: ({ key }) => icon(key),
    preview: ({ key }) => preview(key),
    recent: () => ["lisbon", "code", "budget"],
    open({ key, how }) {
      log(`${how} ${key}`);
      if (key === "cmd:clipboard") return { query: "cb " };
      if (key === "cmd:settings") return { view: "settings" };
      if (how === "copy" && /^(file|folder):/.test(key)) return { note: "Path copied" };
      if (key === "cmd:reindex") return { note: "Looking through your folders again" };
      hide();
      return "done";
    },
    forget: ({ key }) => { clips = clips.filter(c => `clip:${c.id}` !== key); },
    hide: () => hide(),
    fit({ height, wide }) {
      Object.assign(win.style, { width: `${wide ? 1100 : 680}px`, height: `${Math.min(640, Math.max(58, Math.ceil(height)))}px` });
    },
    save_settings({ settings: s }) {
      settings = s;
      applyLook();
      emit("settings", settings);
      return settings;
    },
    set_autostart: ({ enabled }) => (autostart = enabled),
    reindex: () => log("reindex"),
    clear_history: () => log("history cleared"),
    quit: () => log("quit"),
  };

  window.FanlightHost = {
    listen,
    invoke(cmd, args) {
      const fn = commands[cmd];
      if (!fn) return Promise.reject(`unknown command ${cmd}`);
      try { return Promise.resolve(fn(args)); } catch (e) { return Promise.reject(String(e)); }
    },
  };

  // --- the window --------------------------------------------------------------------------------
  function applyLook() {
    document.body.classList.toggle("dark", settings.theme !== "light");
    document.body.classList.toggle("glass", settings.glass);
  }
  function show() {
    win.style.top = `${Math.round(innerHeight * 0.16)}px`;
    win.style.transform = "translateX(-50%)";
    emit("open", null);
    win.classList.add("on");
    frame.contentWindow.focus();
  }
  function hide() {
    win.classList.remove("on");
    emit("closed", null);
  }
  document.addEventListener("pointerdown", e => { if (!win.contains(e.target)) hide(); });
  // Alt+Space on the harness page opens it again, as the shortcut does.
  document.addEventListener("keydown", e => { if (e.altKey && e.code === "Space") { e.preventDefault(); show(); } });

  const wallpaper = data(`<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff9a62"/><stop offset=".45" stop-color="#7b4fc9"/><stop offset="1" stop-color="#1c2a6b"/></linearGradient></defs><rect width="1600" height="900" fill="url(#g)"/><circle cx="1240" cy="160" r="190" fill="#ffd27a" opacity=".85"/><circle cx="260" cy="90" r="130" fill="#5fd0ff" opacity=".7"/><path d="M0 700 Q400 520 800 690 T1600 640 V900 H0Z" fill="#14183a" opacity=".8"/></svg>`);
  document.body.style.setProperty("--wallpaper", `url("${wallpaper}")`);
  applyLook();

  // --- driving the page: typing and keys, for stills and the demo ----------------------------------
  const page = () => frame.contentWindow;
  const field = () => frame.contentDocument.getElementById("q");
  const key = (k, mods = {}) => field().dispatchEvent(new (page().KeyboardEvent)("keydown", { key: k, bubbles: true, cancelable: true, ...mods }));
  const setText = text => { field().value = text; field().dispatchEvent(new (page().Event)("input")); };
  const typeOut = (text, start, each) => [...text].map((_, i) => [start + i * each, () => setText(text.slice(0, i + 1))]);

  frame.addEventListener("load", () => {
    // The page asks for its settings first; then the panel opens, as the shortcut opens it.
    setTimeout(() => {
      if (q.get("demo")) return demo();
      show();
      if (q.get("view") === "settings") return emit("view", "settings");
      const cats = ["apps", "files", "actions", "clipboard"];
      if (q.get("only")) key(String(cats.indexOf(q.get("only")) + 1), { ctrlKey: true });
      if (q.get("q")) setText(q.get("q"));
      setTimeout(() => {
        for (let i = 0; i < Number(q.get("sel") || 0); i++) key("ArrowDown");
        if (q.get("preview")) key(" ", { shiftKey: true });
      }, 300);
    }, 200);
  }, { once: true });
  frame.src = "/ui/index.html";

  // The README's animation (shots.ps1 -Demo): open, find an app, do a sum, find a trip's files and
  // look through them with Space, browse the clipboard, then a command that asks first. Timed from
  // the panel page's load; demo.py records it.
  function demo() {
    const steps = [
      [300, show],
      ...typeOut("vis", 700, 110),
      [2000, () => key("Escape")],
      ...typeOut("(1200+340)*1.2", 2200, 60),
      [3900, () => key("Escape")],
      ...typeOut("lisbon", 4100, 90),
      [5100, () => key("ArrowDown")],
      [5600, () => key(" ")],
      [6700, () => key("ArrowDown")],
      [7600, () => key("ArrowDown")],
      [9000, () => key("Escape")],
      [9300, () => key("Escape")],
      [9700, () => key("4", { ctrlKey: true })],
      [10300, () => key("ArrowDown")],
      [10600, () => key("ArrowDown")],
      [11400, () => key(" ")],
      [12600, () => key("Escape")],
      [12800, () => key("Escape")],
      ...typeOut("restart", 13100, 80),
      [14000, () => key("Enter")],
      [15000, () => key("Escape")],
      [15300, () => key("Escape")],
    ];
    for (const [t, step] of steps) setTimeout(step, t);
  }
})();
