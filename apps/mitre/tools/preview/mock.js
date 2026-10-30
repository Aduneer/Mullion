// Loaded into Mitre's settings page by serve.py, before its script: stands in for Tauri, with a
// small backend of its own.
//
// URL options, all optional:
//   mode=dark|light (otherwise Windows' own)  palette=nord  mullion=1 (started by Mullion)
//   look=1 (Mullion's look for every app)  screens=1|2|3 (default 2)  missing=Bay,Fanlight
(() => {
  const q = new URLSearchParams(location.search);
  const missing = new Set((q.get("missing") || "").split(",").filter(Boolean));
  const ACTIONS = [
    ["nothing", "Nothing", null], ["bay", "Every window (Bay)", "Bay"], ["fanlight", "Search (Fanlight)", "Fanlight"],
    ["apps", "Apps (Fanlight)", "Fanlight"], ["desktop", "Desktop", null], ["task-view", "Task View", null],
    ["lock", "Lock", null], ["screens-off", "Screens off", null],
  ].map(([id, name, app]) => ({ id, name, app, available: !app || !missing.has(app) }));
  // The user's own: a wide main screen at 100%, and one at 125% to its right; or a laptop alone,
  // or with a screen on each side.
  const SCREENS = {
    1: [["a", [0, 0, 1920, 1200], 150, true]],
    2: [["a", [0, 0, 2560, 1080], 100, true], ["b", [2560, 0, 1920, 1080], 125, false]],
    3: [["c", [-1920, 0, 1920, 1080], 100, false], ["a", [0, 0, 2560, 1440], 125, true], ["b", [2560, -180, 1080, 1920], 100, false]],
  }[q.get("screens") || "2"] ?? [];
  const inside = (x, y) => SCREENS.some(([, r]) => x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]);
  const hard = r => [[r[0], r[1], -1, -1], [r[0] + r[2] - 1, r[1], 1, -1], [r[0], r[1] + r[3] - 1, -1, 1], [r[0] + r[2] - 1, r[1] + r[3] - 1, 1, 1]]
    .map(([x, y, dx, dy]) => !inside(x + dx, y) && !inside(x, y + dy) && !inside(x + dx, y + dy));
  const screens = [...SCREENS].sort((a, b) => a[1][0] - b[1][0]).map(([key, rect, scale, main], i) => ({ key, name: `Screen ${i + 1}`, main, rect, scale, hard: hard(rect) }));
  let settings = {
    theme: q.get("mode") || "auto", palette: q.get("palette") || "oriel", dwell: 150, glow: true,
    corners: { a: ["bay", "nothing", "desktop", "apps"], b: ["nothing", "task-view", "nothing", "lock"] },
  };
  let autostart = true;
  const commands = {
    hello: () => ({
      settings, actions: ACTIONS, screens, dwells: [100, 150, 250, 400, 600], autostart, mullion: q.get("mullion") === "1",
      shared_look: q.get("look") === "1", icon: "/src-tauri/icons/128x128.png", version: "0.4.1",
      settings_path: String.raw`C:\Users\you\AppData\Roaming\Mitre\settings.json`,
    }),
    save_settings: ({ settings: s }) => (settings = s),
    set_autostart: ({ enabled }) => (autostart = enabled),
    get_apps: () => console.log("would open GitHub's releases page in the browser"),
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args = {}) => (commands[cmd] ?? (() => null))(args) },
    event: { listen: () => Promise.resolve(() => {}) },
  };
})();
