// Loaded into Sash's pages by serve.py, before their scripts: stands in for Tauri. The settings
// page and the note each have a small backend of their own here.
//
// URL options, all optional:
//   mode=dark|light (otherwise Windows' own)  palette=nord  mullion=1 (started by Mullion)
//   taken=1 (another app has Right half's shortcut)  text=… (what the note says)
(() => {
  const q = new URLSearchParams(location.search);
  const ACTIONS = [
    ["left", "Left half", "Halves", "ctrl+alt+left"], ["right", "Right half", "Halves", "ctrl+alt+right"],
    ["top", "Top half", "Halves", "ctrl+alt+up"], ["bottom", "Bottom half", "Halves", "ctrl+alt+down"],
    ["maximize", "Maximize", "The whole screen", "ctrl+alt+enter"], ["centre", "Centre", "The whole screen", "ctrl+alt+space"],
    ["restore", "Restore", "The whole screen", "ctrl+alt+backspace"], ["next-screen", "Next screen", "The whole screen", "ctrl+alt+shift+right"],
    ["previous-screen", "Previous screen", "The whole screen", "ctrl+alt+shift+left"],
    ["first-third", "Left third", "Thirds", ""], ["middle-third", "Middle third", "Thirds", ""], ["last-third", "Right third", "Thirds", ""],
    ["first-two-thirds", "Left two-thirds", "Thirds", ""], ["last-two-thirds", "Right two-thirds", "Thirds", ""],
    ["top-left", "Top-left quarter", "Quarters", ""], ["top-right", "Top-right quarter", "Quarters", ""],
    ["bottom-left", "Bottom-left quarter", "Quarters", ""], ["bottom-right", "Bottom-right quarter", "Quarters", ""],
  ].map(([id, name, group, key]) => ({ id, name, group, key }));
  let settings = {
    theme: q.get("mode") || "auto", palette: q.get("palette") || "oriel", cycle: true, gap: Number(q.get("gap") || 0),
    shortcuts: Object.fromEntries(ACTIONS.map(a => [a.id, a.key])),
  };
  let autostart = true;
  const listeners = {};
  const commands = {
    hello: () => ({
      settings, actions: ACTIONS, errors: q.get("taken") === "1" ? { right: "another app already uses that shortcut" } : {}, gaps: [0, 4, 8, 16],
      autostart, mullion: q.get("mullion") === "1", icon: "/src-tauri/icons/128x128.png", version: "0.4.1",
      settings_path: String.raw`C:\Users\you\AppData\Roaming\Sash\settings.json`,
    }),
    set_shortcut: ({ action, spec }) => {
      const other = ACTIONS.find(a => a.id !== action && spec && settings.shortcuts[a.id] === spec);
      if (other) throw `${spec} is already ${other.name}`;
      settings = { ...settings, shortcuts: { ...settings.shortcuts, [action]: spec } };
      return settings;
    },
    save_settings: ({ settings: s }) => (settings = { ...s, shortcuts: settings.shortcuts }),
    set_autostart: ({ enabled }) => (autostart = enabled),
    note_hello: () => ({
      theme: q.get("mode") || "dark", palette: q.get("palette") || "oriel", icon: "/src-tauri/icons/128x128.png",
      text: q.get("text") || "“Task Manager” is running as administrator, so Windows won’t let Sash move it.",
    }),
    // In the harness the note sits in a frame its own size: tell the page around it.
    note_ready: ({ w, h }) => window.parent?.postMessage({ note: { w, h } }, "*"),
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args = {}) => (commands[cmd] ?? (() => null))(args) },
    event: { listen: (event, fn) => Promise.resolve((listeners[event] ??= []).push(fn)) },
  };
})();
