// Loaded into Apron's dock and settings pages by serve.py, before their scripts: stands in for
// Tauri, with a small backend of its own. In the harness (index.html) the dock is a frame, and asks
// the page around it (host.js) where it is and how big.
//
// URL options (the harness page's, or the settings page's own), all optional:
//   mode=dark|light (otherwise Windows' own)  palette=nord  edge=bottom|left|right
//   size=small|medium|large  float=1  glass=1  mullion=1 (started by Mullion)
//   look=1 (Mullion's look for every app)  fanlight=0 (not installed)  tray=0 (no icons)
//   screens=1..4 (how many screens; settings)  screen=\\.\DISPLAY9 (the one chosen; settings)
(() => {
  const host = window.parent !== window ? window.parent.ApronHost : null;
  const q = new URLSearchParams((host ? window.parent : window).location.search);
  const svg = body => "data:image/svg+xml;base64," + btoa(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">${body}</svg>`);
  // Made-up apps, drawn as tiles with a mark on them.
  const tile = (a, b, mark) => svg(`<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/></linearGradient></defs><rect x="4" y="4" width="56" height="56" rx="14" fill="url(#g)"/>${mark}`);
  const W = (stroke = "#fff", width = 4) => `fill="none" stroke="${stroke}" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round"`;
  const ICONS = {
    files: tile("#ffd36b", "#f0a92e", `<path d="M16 22h12l4 4h16v18H16z" ${W()}/>`),
    browser: tile("#5aa0f2", "#2f6fd1", `<circle cx="32" cy="32" r="15" ${W()}/><path d="M17 32h30M32 17c-7 8-7 22 0 30M32 17c7 8 7 22 0 30" ${W("#fff", 3)}/>`),
    mail: tile("#7ad0ff", "#3a8fd8", `<rect x="16" y="21" width="32" height="22" rx="3" ${W()}/><path d="M17 23l15 11 15-11" ${W()}/>`),
    terminal: tile("#4a4f5a", "#2a2d33", `<path d="M18 24l8 8-8 8M30 41h14" ${W()}/>`),
    notes: tile("#ffe58a", "#f5c843", `<path d="M20 22h24M20 31h24M20 40h14" ${W("#8a6a12")}/>`),
    music: tile("#ff7a8a", "#e0405f", `<path d="M27 42V22l16-3v20" ${W()}/><circle cx="23" cy="42" r="4" fill="#fff"/><circle cx="39" cy="39" r="4" fill="#fff"/>`),
    photos: tile("#8be08b", "#3fae6a", `<path d="M16 42l10-12 8 9 5-6 9 9z" fill="#fff"/><circle cx="40" cy="23" r="4" fill="#fff"/>`),
    code: tile("#6b7cff", "#4048d0", `<path d="M25 22l-9 10 9 10M39 22l9 10-9 10" ${W()}/>`),
  };
  const app = (key, name, pinned, windows = []) => ({ key, name, icon: ICONS[key], pinned, windows: windows.map((w, i) => ({ hwnd: 100 + i, pid: 4000, title: `${name} ${i + 1}`, front: false, minimized: false, ...w })) });
  let items = [
    app("files", "Files", true, [{}]),
    app("browser", "Browser", true, [{ front: true }, {}]),
    app("mail", "Mail", true),
    app("terminal", "Terminal", true, [{}]),
    app("notes", "Notes", true),
    app("music", "Music", true, [{ minimized: true }]),
    app("code", "Code", false, [{}, {}, {}]),
    app("photos", "Photos", false, [{}]),
  ];
  // The notification area: small marks, as apps draw them for Windows' own light or dark.
  const ink = q.get("mode") === "light" ? "#1f1f1f" : "#e8e8e8";
  const I = `fill="none" stroke="${ink}" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"`;
  const tray = [
    ["cloud", "Files are up to date", "sync.exe", `<path d="M18 46h28a10 10 0 0 0 0-20 14 14 0 0 0-27-2 11 11 0 0 0-1 22z" ${I}/>`],
    ["chat", "Chat: 2 unread", "chat.exe", `<path d="M12 16h40v26H30l-10 8v-8h-8z" ${I}/><circle cx="50" cy="16" r="9" fill="#e0405f"/>`],
    ["shield", "Security: no actions needed", "guard.exe", `<path d="M32 8l20 7v14c0 13-9 22-20 27-11-5-20-14-20-27V15z" ${I}/><path d="M23 32l7 7 12-13" ${I}/>`],
    ["net", "Network: connected", "explorer.exe", `<path d="M10 26a32 32 0 0 1 44 0M17 34a22 22 0 0 1 30 0M24 42a12 12 0 0 1 16 0" ${I}/><circle cx="32" cy="50" r="3.5" fill="${ink}"/>`],
    ["sound", "Speakers: 64%", "explorer.exe", `<path d="M12 26h9l12-10v32L21 38h-9z" ${I}/><path d="M42 24a11 11 0 0 1 0 16M48 18a19 19 0 0 1 0 28" ${I}/>`],
    ["printer", "Printer", "print.exe", `<rect x="12" y="24" width="40" height="18" rx="3" ${I}/>`, false],
    ["update", "Updates are ready", "update.exe", `<path d="M32 12v28M20 28l12 12 12-12M14 52h36" ${I}/>`, false],
  ].map(([id, tip, exe, body, promoted = true]) => ({ id, tip, exe, icon: svg(body), promoted }));

  // The Start menu's apps, for Add.
  const START = [["code", "Code"], ["files", "Files"], ["mail", "Mail"], ["music", "Music"], ["notes", "Notes"], ["photos", "Photos"], ["terminal", "Terminal"], ["browser", "Browser"]]
    .map(([key, name]) => ({ key, name, launch: `C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\${name}.lnk`, icon: ICONS[key] }))
    .sort((a, b) => a.name.localeCompare(b.name));

  let settings = {
    theme: q.get("mode") || "auto", palette: q.get("palette") || "oriel", glass: q.get("glass") === "1",
    edge: q.get("edge") || "bottom", size: q.get("size") || "medium", float: q.get("float") === "1",
    screen: q.get("screen") || "",
  };
  let autostart = true;
  const listeners = {};
  const emit = (event, payload) => (listeners[event] ?? []).forEach(fn => fn({ event, payload }));
  const log = text => (host ? host.log(text) : console.log(text));
  const find = key => items.find(a => a.key === key);
  const commands = {
    hello: () => ({
      settings, items, tray: q.get("tray") === "0" ? [] : tray, fanlight: q.get("fanlight") !== "0", autostart,
      mullion: q.get("mullion") === "1", shared_look: q.get("look") === "1", icon: "/src-tauri/icons/128x128.png",
      version: "0.5.0", settings_path: String.raw`C:\Users\you\AppData\Roaming\Apron\settings.json`,
      screens: [
        { id: "", name: "Screen 1 (main)", size: "2560 × 1080" },
        { id: String.raw`\\.\DISPLAY2`, name: "Screen 2", size: "1920 × 1080" },
        { id: String.raw`\\.\DISPLAY3`, name: "Screen 3", size: "1080 × 1920" },
        { id: String.raw`\\.\DISPLAY4`, name: "Screen 4", size: "1920 × 1200" },
      ].slice(0, Number(q.get("screens") || 1)),
    }),
    activate({ key }) {
      const a = find(key);
      if (a.windows.length > 1) return (log(`${a.name}'s windows: ${a.windows.map(w => w.title).join(", ")}`), "menu");
      if (!a.windows.length) {
        // As if it opened a moment later.
        setTimeout(() => { a.windows = [{ hwnd: 900, pid: 4100, title: a.name, front: true, minimized: false }]; emit("items", items); }, 1400);
        return "launch";
      }
      const front = a.windows.find(w => w.front);
      items.forEach(o => o.windows.forEach(w => { w.front = false; }));
      if (front) front.minimized = true;
      else Object.assign(a.windows[0], { front: true, minimized: false });
      setTimeout(() => emit("items", items));
      return front ? "minimize" : "focus";
    },
    new_window: ({ key }) => log(`new window: ${key}`),
    app_menu: ({ key, x, y }) => log(`${find(key).name}'s menu at ${Math.round(x)}, ${Math.round(y)}`),
    dock_menu: ({ x, y }) => log(`Apron's menu at ${Math.round(x)}, ${Math.round(y)}`),
    reorder: ({ keys }) => log(`order: ${keys.join(", ")}`),
    open_grid: () => log("Fanlight's apps"),
    tray_click: ({ id, kind }) => log(`${kind} click: ${id}`),
    tray_more: () => log(`more: ${tray.filter(t => !t.promoted).map(t => t.tip).join(", ")}`),
    shelf: ({ width, height }) => host?.shelf(width, height),
    wallpaper: () => host?.wallpaper() ?? null,
    wallpaper_image: () => host?.wallpaperArt ?? null,
    save_settings: ({ settings: s }) => (settings = s),
    set_autostart: ({ enabled }) => (autostart = enabled),
    get_apps: () => log("would open GitHub's releases page in the browser"),
    unpin({ key }) {
      items = items.filter(a => a.key !== key || a.windows.length).map(a => (a.key === key ? { ...a, pinned: false } : a));
      emit("items", items);
    },
    pin_paths({ paths }) {
      for (const p of paths) {
        const s = START.find(a => a.launch === p);
        const a = find(s.key);
        if (a) a.pinned = true;
        else items.push({ key: s.key, name: s.name, icon: s.icon, pinned: true, windows: [] });
      }
      // Pinned apps first, as Apron lists them.
      items = [...items.filter(a => a.pinned), ...items.filter(a => !a.pinned)];
      emit("items", items);
    },
    start_menu_apps: () => new Promise(ok => setTimeout(() => ok(START), 500)),
    picker_show: ({ width, height }) => log(`picker shown at ${width}×${height}`),
    picker_pick: ({ hwnd }) => log(`picked window ${hwnd}`),
    picker_close: ({ hwnd }) => log(`closed window ${hwnd}`),
    picker_hide: ({ back }) => log(`picker put away${back ? ", focus given back" : ""}`),
  };
  // The picker on its own: as if an app with several windows was clicked in the dock.
  if (location.pathname.endsWith("/picker.html")) {
    const titles = ["Home - File Explorer", "dev and 1 more tab - File Explorer", "Downloads - File Explorer", "A rather long folder name that goes on for a while - File Explorer"];
    addEventListener("load", () => emit("pick", {
      name: "File Explorer", icon: ICONS.files, edge: settings.edge, theme: settings.theme, palette: settings.palette,
      windows: titles.slice(0, Number(q.get("n") || 3)).map((title, i) => ({ hwnd: 100 + i, pid: 4000, title, front: i === 1, minimized: i === 2 })),
    }));
  }
  const listen = (event, fn) => { (listeners[event] ??= []).push(fn); return Promise.resolve(() => {}); };
  window.ApronMock = { emit };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args = {}) => (commands[cmd] ?? (() => null))(args) },
    event: { listen },
    webviewWindow: { getCurrentWebviewWindow: () => ({ listen }) },
  };
})();
