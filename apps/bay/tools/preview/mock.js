// Loaded into Bay's pages by serve.py, before their scripts: stands in for Tauri. In the harness
// (tools/preview/index.html) the overview passes everything to the page around it (host.js); the
// settings page, opened by itself, has a small backend of its own here.
//
// The settings page's URL options, all optional:
//   mode=dark|light (otherwise Windows' own)  palette=nord  mullion=1 (started by Mullion)
(() => {
  const host = window.parent !== window ? window.parent.BayHost : null;
  if (host) {
    window.__TAURI__ = {
      core: { invoke: (cmd, args = {}) => host.invoke(cmd, args) },
      webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (event, fn) => Promise.resolve(host.listen(event, payload => fn({ event, payload }))) }) },
    };
    return;
  }
  const q = new URLSearchParams(location.search);
  let settings = { theme: q.get("mode") || "auto", palette: q.get("palette") || "oriel", hotkey: "win+ctrl+up", backdrop: "wallpaper", minimized: true };
  let autostart = true;
  const commands = {
    hello: () => ({
      settings, hotkey_error: null, hotkeys: ["win+ctrl+up", "win+shift+tab", "ctrl+alt+up", "ctrl+up"], autostart,
      mullion: q.get("mullion") === "1", icon: "/src-tauri/icons/128x128.png", version: "0.3.1",
      settings_path: String.raw`C:\Users\you\AppData\Roaming\Bay\settings.json`,
    }),
    save_settings: ({ settings: s }) => (settings = s),
    set_autostart: ({ enabled }) => (autostart = enabled),
  };
  window.__TAURI__ = { core: { invoke: async (cmd, args = {}) => (commands[cmd] ?? (() => null))(args) } };
})();
