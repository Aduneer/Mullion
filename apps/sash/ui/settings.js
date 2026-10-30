"use strict";
// Sash's settings: a shortcut for each thing it does, how it snaps, its look, and its sign-in
// entry. Each change is saved as it's made and works at once.
{
const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const $ = id => document.getElementById(id);
const setv = $("settings");

// The action whose shortcut is being listened for, if any.
let listening = null;

// A settings window, not a web page: no browser right-click menu, and none of its keys.
document.addEventListener("contextmenu", e => e.preventDefault());
document.addEventListener("keydown", e => {
  if (listening) return;
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
const GAP_NAMES = { 0: "None", 4: "Small", 8: "Medium", 16: "Large" };
const KEYS = { up: "↑", down: "↓", left: "←", right: "→", pageup: "PgUp", pagedown: "PgDn", backspace: "Backspace", enter: "Enter" };
const keyName = p => KEYS[p] ?? p[0].toUpperCase() + p.slice(1);
const kbds = spec => spec.split("+").map(p => `<kbd>${esc(keyName(p))}</kbd>`).join("");
const label = spec => spec.split("+").map(keyName).join("+");

// The key a keydown names, as Sash writes it, from where the key is rather than what it types
// (so AltGr+E is "e", not "€"). Null for a key that can't be part of a shortcut.
function keyOf(e) {
  const c = e.code;
  if (c.startsWith("Arrow")) return c.slice(5).toLowerCase();
  if (/^Key[A-Z]$/.test(c)) return c.slice(3).toLowerCase();
  if (/^Digit\d$/.test(c)) return c.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(c)) return c.toLowerCase();
  return { Space: "space", Tab: "tab", Enter: "enter", NumpadEnter: "enter", Backspace: "backspace", Delete: "delete",
    Insert: "insert", Home: "home", End: "end", PageUp: "pageup", PageDown: "pagedown" }[c] ?? null;
}

// Ctrl+Alt and a key that types is AltGr and that key on many keyboards.
const altgr = spec => /^ctrl\+alt\+(shift\+)?[a-z0-9]$/.test(spec);

let hello = null, settings = null;
const errors = {};

const dark = matchMedia("(prefers-color-scheme: dark)");
function applyTheme() {
  const root = document.documentElement;
  root.dataset.palette = settings?.palette ?? "oriel";
  const theme = settings?.theme ?? "auto";
  root.dataset.mode = theme === "auto" ? (dark.matches ? "dark" : "light") : theme;
}
dark.addEventListener("change", applyTheme);

const sw = (attrs, on, name) => `<button class="switch" role="switch" ${attrs} aria-checked="${on}" aria-label="${esc(name)}"></button>`;
const seg = (key, options, current) => `<div class="seg">${options.map(([v, t]) => `<button data-${key}="${v}" aria-pressed="${String(current) === String(v)}">${t}</button>`).join("")}</div>`;
const setRow = (title, sub, control) => `<div class="set"><div class="label"><span>${title}</span>${sub ? `<small>${sub}</small>` : ""}</div>${control}</div>`;

function shortcutRow(a) {
  const spec = settings.shortcuts[a.id] ?? "";
  const on = listening === a.id;
  const sub = errors[a.id] ? `<span class="err">${esc(errors[a.id])}</span>`
    : on ? "Press the keys, <kbd>Esc</kbd> to keep it, <kbd>Delete</kbd> for none"
    : altgr(spec) ? `<span class="warn">On keyboards with AltGr, this is also AltGr+${esc(keyName(spec.split("+").pop()))}</span>` : "";
  const keys = on ? "Press keys…" : spec ? kbds(spec) : "None";
  return setRow(esc(a.name), sub, `<button class="keys${spec || on ? "" : " none"}${on ? " listening" : ""}" data-keys="${esc(a.id)}" aria-label="${esc(a.name)}: ${esc(spec ? label(spec) : "none")}">${keys}</button>`);
}

function draw() {
  const s = settings;
  const groups = [];
  for (const a of hello.actions) {
    let g = groups.find(g => g.name === a.group);
    if (!g) groups.push(g = { name: a.group, actions: [] });
    g.actions.push(a);
  }
  setv.innerHTML = `
    ${groups.map((g, i) => `<h3>${esc(g.name.toUpperCase())}</h3>
      ${i === 0 ? `<p class="hint">The window in front goes to that half. Press the same arrow again for two-thirds, then a third; a half, then an arrow the other way, makes a quarter.</p>` : ""}
      ${g.actions.map(shortcutRow).join("")}`).join("")}
    <h3>HOW IT SNAPS</h3>
    ${setRow("Pressing again", s.cycle ? "Two-thirds, then a third, then half again" : "Stays a half", sw(`data-set="cycle"`, s.cycle, "Pressing again"))}
    ${setRow("Space between windows", "And around the screen’s edges", seg("gap", hello.gaps.map(g => [g, GAP_NAMES[g] ?? `${g} px`]), s.gap))}
    ${setRow("Windows’ own Snap", "<kbd>Win</kbd> + arrows, and dragging a window to an edge, are Windows’ own and work beside Sash", `<button class="btn" data-do="snap">Open its settings</button>`)}
    <h3>LOOK</h3>
    ${hello.shared_look
      ? setRow("Theme and palette", "Mullion Settings sets the look for every app", "")
      : setRow("Theme", "", seg("theme", [["auto", "Auto"], ["dark", "Dark"], ["light", "Light"]], s.theme))
        + setRow("Palette", esc(PALETTES.find(p => p[0] === s.palette)?.[1] ?? ""), `<div class="swatches">${PALETTES.map(([id, name, c]) => `<button class="swatch" data-palette="${id}" title="${name}" aria-label="${name}" aria-pressed="${s.palette === id}" style="background-color:${c}"></button>`).join("")}</div>`)}
    <h3>GENERAL</h3>
    ${hello.mullion
      ? setRow("Start at sign-in", "Mullion starts Sash: see Mullion Settings", "")
      : setRow("Start at sign-in", "", sw(`data-do="autostart"`, hello.autostart, "Start at sign-in"))}
    ${setRow("Quit Sash", "Until you start it again", `<button class="btn danger" data-do="quit">Quit</button>`)}
    <p class="about">Sash ${esc(hello.version)} · part of the Mullion suite, with Oriel, Lintel, Fanlight and Bay<br>Settings: ${esc(hello.settings_path)}</p>`;
  if (listening) setv.querySelector(`[data-keys="${CSS.escape(listening)}"]`)?.focus();
}

async function save(change) {
  try {
    settings = await invoke("save_settings", { settings: { ...settings, ...change } });
    applyTheme();
  } catch (e) {
    console.error(e);
  }
  draw();
}

// Listening for a shortcut: Sash lets go of its own meanwhile, so it hears them too.
async function listen(id) {
  if (listening === id) return stop();
  if (!listening) await invoke("recording", { on: true });
  listening = id;
  delete errors[id];
  draw();
}

async function stop() {
  if (!listening) return;
  listening = null;
  await invoke("recording", { on: false });
  draw();
}

async function setShortcut(id, spec) {
  listening = null;
  try {
    settings = await invoke("set_shortcut", { action: id, spec });
    delete errors[id];
  } catch (e) {
    errors[id] = String(e);
  }
  draw();
}

document.addEventListener("keydown", e => {
  if (!listening) return;
  e.preventDefault();
  e.stopPropagation();
  const mods = [e.ctrlKey && "ctrl", e.altKey && "alt", e.shiftKey && "shift", e.metaKey && "win"].filter(Boolean);
  if (e.key === "Escape" && !mods.length) return stop();
  if ((e.key === "Delete" || e.key === "Backspace") && !mods.length) return setShortcut(listening, "");
  const key = keyOf(e);
  if (!key) return;
  if (!mods.length) {
    errors[listening] = "Add Ctrl, Alt, Shift or Win to it";
    return draw();
  }
  setShortcut(listening, [...mods, key].join("+"));
}, true);
// Clicking away, or into another window, stops listening.
window.addEventListener("blur", stop);
document.addEventListener("mousedown", e => { if (listening && !e.target.closest("[data-keys]")) stop(); });

setv.addEventListener("click", async e => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.dataset.keys) return listen(t.dataset.keys);
  if (t.dataset.set) return save({ [t.dataset.set]: t.getAttribute("aria-checked") !== "true" });
  if (t.dataset.gap) return save({ gap: Number(t.dataset.gap) });
  if (t.dataset.theme) return save({ theme: t.dataset.theme });
  if (t.dataset.palette) return save({ palette: t.dataset.palette });
  if (t.dataset.do === "snap") return invoke("snap_settings");
  if (t.dataset.do === "autostart") {
    hello.autostart = await invoke("set_autostart", { enabled: !hello.autostart }).catch(() => hello.autostart);
    return draw();
  }
  if (t.dataset.do === "quit") invoke("quit");
});

// Mullion's look for every app changed, or went on or off.
T.event.listen("look", e => { if (hello) hello.shared_look = e.payload; });
T.event.listen("settings", e => { if (!hello) return; settings = e.payload; applyTheme(); draw(); });

invoke("hello").then(h => {
  hello = h;
  settings = h.settings;
  Object.assign(errors, h.errors);
  $("logo").innerHTML = `<img src="${h.icon}" alt="">`;
  applyTheme();
  draw();
  invoke("settings_ready");
});
}
