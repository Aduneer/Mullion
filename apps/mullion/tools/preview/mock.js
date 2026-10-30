// Loaded into the settings page by serve.py, before ui/settings.js: stands in for Tauri with
// made-up apps, so the page can be worked on (and photographed) without building anything.
//
// URL options, all optional:
//   mode=dark|light (otherwise Windows' own)  off=Lintel (turned off)  missing=Oriel (not installed)
//   welcome=1 (the first time, before Start)  update=1 (a newer Mullion is out)
(() => {
  const q = new URLSearchParams(location.search);
  if (q.get("mode")) {
    const dark = q.get("mode") === "dark";
    const real = window.matchMedia.bind(window);
    window.matchMedia = query => query.includes("prefers-color-scheme")
      ? { matches: query.includes("dark") === dark, addEventListener() {} }
      : real(query);
  }
  const list = n => (q.get(n) || "").split(",").filter(Boolean);
  const off = new Set(list("off")), missing = new Set(list("missing"));
  let autostart = true, welcomed = q.get("welcome") !== "1", checkUpdates = true;
  const updates = q.get("update") === "1"
    ? { checking: false, installing: false, checked: Date.now() / 1000 - 600, can_install: true,
        newer: { version: "0.5.0", page: "#", notes: "**Sash**: windows snap to halves, thirds and quarters.\n\nMore below." } }
    : { checking: false, installing: false, checked: Date.now() / 1000 - 600, can_install: true, newer: null };
  const APPS = [
    ["Oriel", "A command centre for your second screen", true],
    ["Lintel", "A menu bar along the top of the screen", false],
    ["Fanlight", "Search and launch on Alt+Space", true],
    ["Bay", "Every open window at once, on Win+Ctrl+Up", true],
    ["Sash", "Windows snapped into place, on Ctrl+Alt+arrows", true],
  ];
  const apps = icons => APPS.map(([name, about, shows]) => {
    const installed = !missing.has(name), on = !off.has(name);
    const a = { name, about, shows, installed, on, running: installed && on };
    if (icons && installed) a.icon = `/icons/${name.toLowerCase()}.png`;
    return a;
  });
  const commands = {
    hello: () => ({ apps: apps(true), icon: "/icons/mullion.png", autostart, welcomed, check_updates: checkUpdates, updates, version: "0.1.0", settings_path: String.raw`C:\Users\you\AppData\Roaming\Mullion\settings.json` }),
    apps: () => apps(false),
    set_app: ({ name, on }) => { (on ? off.delete(name) : off.add(name)); return apps(false); },
    welcome: ({ off: picked }) => { off.clear(); picked.forEach(n => off.add(n)); welcomed = true; },
    set_autostart: ({ enabled }) => (autostart = enabled),
    set_check_updates: ({ on }) => (checkUpdates = on),
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args = {}) => (commands[cmd] ?? (() => null))(args) },
    event: { listen: async () => () => {} },
  };
})();
