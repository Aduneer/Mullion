"use strict";
// Bay's settings: its shortcut, its look, whether minimized windows show, and its sign-in entry.
// Each change is saved as it's made; the overview takes it the next time it opens.
{
const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const $ = id => document.getElementById(id);
const setv = $("settings");

// A settings window, not a web page: no browser right-click menu, and none of its keys.
document.addEventListener("contextmenu", e => e.preventDefault());
document.addEventListener("keydown", e => {
  const k = e.key.toLowerCase();
  if (["f5", "f7", "f12", "browserback", "browserforward"].includes(k)
    || (e.ctrlKey && ["r", "f", "g", "p", "u", "j", "+", "-", "=", "0"].includes(k))
    || (e.ctrlKey && e.shiftKey && ["i"].includes(k))
    || (e.altKey && ["arrowleft", "arrowright"].includes(k))) e.preventDefault();
}, true);
document.addEventListener("wheel", e => { if (e.ctrlKey) e.preventDefault(); }, { passive: false });

const esc = s => String(s ?? "").replace(/[&<>"']/g, c => `&#${c.charCodeAt(0)};`);
const PALETTES = [
  ["oriel", "Default", "#3987e5"], ["catppuccin", "Catppuccin", "#cba6f7"], ["nord", "Nord", "#88c0d0"],
  ["gruvbox", "Gruvbox", "#8ec07c"], ["rose-pine", "Rosé Pine", "#c4a7e7"], ["solarized", "Solarized", "#2aa198"],
];
const KEYS = { up: "↑", down: "↓", left: "←", right: "→" };
const label = spec => spec.split("+").map(p => KEYS[p] ?? p[0].toUpperCase() + p.slice(1)).join(" + ");

let hello = null, settings = null;

const dark = matchMedia("(prefers-color-scheme: dark)");
function applyTheme() {
  const root = document.documentElement;
  root.dataset.palette = settings?.palette ?? "oriel";
  const theme = settings?.theme ?? "auto";
  root.dataset.mode = theme === "auto" ? (dark.matches ? "dark" : "light") : theme;
}
dark.addEventListener("change", applyTheme);

const sw = (attrs, on, name) => `<button class="switch" role="switch" ${attrs} aria-checked="${on}" aria-label="${esc(name)}"></button>`;
const opt = (value, text, current) => `<option value="${esc(value)}"${value === current ? " selected" : ""}>${esc(text)}</option>`;
const seg = (key, options, current) => `<div class="seg">${options.map(([v, t]) => `<button data-${key}="${v}" aria-pressed="${current === v}">${t}</button>`).join("")}</div>`;
const setRow = (title, sub, control) => `<div class="set"><div class="label"><span>${title}</span>${sub ? `<small>${sub}</small>` : ""}</div>${control}</div>`;

function draw(error = hello.hotkey_error) {
  const s = settings;
  setv.innerHTML = `
    <h3>SHORTCUT</h3>
    ${setRow("Show every window with", error ? `<span class="err">${esc(error)}</span>`
      : hello.mullion ? "Or from Mullion’s icon in the notification area" : "Or click Bay’s icon in the notification area",
      `<select data-set="hotkey">${hello.hotkeys.map(h => opt(h, label(h), s.hotkey)).join("")}</select>`)}
    ${setRow("In the overview", "<kbd>←</kbd> <kbd>→</kbd> <kbd>↑</kbd> <kbd>↓</kbd> or <kbd>Tab</kbd> to move, <kbd>Enter</kbd> to pick, <kbd>Esc</kbd> to go back. Drag a window onto another screen to move it there", "")}
    <h3>LOOK</h3>
    ${setRow("Theme", "", seg("theme", [["auto", "Auto"], ["dark", "Dark"], ["light", "Light"]], s.theme))}
    ${setRow("Palette", esc(PALETTES.find(p => p[0] === s.palette)?.[1] ?? ""), `<div class="swatches">${PALETTES.map(([id, name, c]) => `<button class="swatch" data-palette="${id}" title="${name}" aria-label="${name}" aria-pressed="${s.palette === id}" style="background-color:${c}"></button>`).join("")}</div>`)}
    ${setRow("Behind the windows", s.backdrop === "wallpaper" ? "Your wallpaper, blurred and dimmed" : "The palette’s own colour", seg("backdrop", [["wallpaper", "Wallpaper"], ["plain", "Plain"]], s.backdrop))}
    <h3>WINDOWS</h3>
    ${setRow("Minimized windows", "As cards with their icon, among the others", sw(`data-set="minimized"`, s.minimized, "Minimized windows"))}
    <h3>GENERAL</h3>
    ${hello.mullion
      ? setRow("Start at sign-in", "Mullion starts Bay: see Mullion Settings", "")
      : setRow("Start at sign-in", "", sw(`data-do="autostart"`, hello.autostart, "Start at sign-in"))}
    ${setRow("Quit Bay", "Until you start it again", `<button class="btn danger" data-do="quit">Quit</button>`)}
    <p class="about">Bay ${esc(hello.version)} · part of the Mullion suite, with Oriel, Lintel, Fanlight and Sash<br>Settings: ${esc(hello.settings_path)}</p>`;
}

async function save(change) {
  try {
    settings = await invoke("save_settings", { settings: { ...settings, ...change } });
    hello.hotkey_error = null;
    applyTheme();
    draw(null);
  } catch (e) {
    draw(String(e));
  }
}

setv.addEventListener("click", async e => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.dataset.set) return save({ [t.dataset.set]: t.getAttribute("aria-checked") !== "true" });
  if (t.dataset.theme) return save({ theme: t.dataset.theme });
  if (t.dataset.palette) return save({ palette: t.dataset.palette });
  if (t.dataset.backdrop) return save({ backdrop: t.dataset.backdrop });
  if (t.dataset.do === "autostart") {
    hello.autostart = await invoke("set_autostart", { enabled: !hello.autostart }).catch(() => hello.autostart);
    return draw();
  }
  if (t.dataset.do === "quit") invoke("quit");
});
setv.addEventListener("change", e => {
  if (e.target.dataset.set) save({ [e.target.dataset.set]: e.target.value });
});

invoke("hello").then(h => {
  hello = h;
  settings = h.settings;
  $("logo").innerHTML = `<img src="${h.icon}" alt="">`;
  applyTheme();
  draw();
  invoke("settings_ready");
});
}
