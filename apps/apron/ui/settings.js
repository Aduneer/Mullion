"use strict";
// Apron's settings: which edge the dock sits on and how big it is, whether it takes the taskbar's
// place or floats above it, whether it keeps the notification area's icons while Lintel shows
// them, its look, and its sign-in entry. Each change is saved as it's made and
// works at once.
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
// On a button that opens a page in the browser, so it never does that unannounced.
const OUT = `<svg class="ic out" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 5h5v5M19 5l-8 8M17 14v4a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1h4"/></svg>`;

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
const seg = (key, options, current) => `<div class="seg">${options.map(([v, t]) => `<button data-${key}="${v}" aria-pressed="${String(current) === String(v)}">${t}</button>`).join("")}</div>`;
const setRow = (title, sub, control) => `<div class="set"><div class="label"><span>${title}</span>${sub ? `<small>${sub}</small>` : ""}</div>${control}</div>`;

function draw() {
  const s = settings;
  setv.innerHTML = `
    <h3>POSITION</h3>
    ${screenRow()}
    ${setRow("Edge", hello.screens.length > 1 ? "Of its screen" : "Of the main screen", seg("edge", [["bottom", "Bottom"], ["left", "Left"], ["right", "Right"]], s.edge))}
    ${setRow("Size", "Of the app icons", seg("size", [["small", "Small"], ["medium", "Medium"], ["large", "Large"]], s.size))}
    ${setRow("Float and auto-hide", s.float
      ? "Windows’ taskbar stays; Apron floats above it while the pointer is at the edge"
      : "Off, Apron takes the taskbar’s place and hides it until Apron quits", sw(`data-set="float"`, s.float, "Float and auto-hide"))}
    <p class="hint" style="padding-top:8px">Click an app to bring it forward, again to minimize it, or pick a window when it has several; middle-click for a new window; right-click for its windows, Pin and Quit. Drag pinned apps to put them in order.</p>
    ${hello.fanlight ? "" : setRow("Fanlight isn’t installed", "The dock’s first button shows your apps once it is",
      `<button class="btn" data-do="get" title="Opens GitHub in your browser">Get it${OUT}</button>`)}
    <h3>NOTIFICATION AREA</h3>
    ${setRow("Keep the icons in the dock too", hello.lintel_tray
      ? (s.tray_too ? "Lintel shows them on its bar, and so does the dock" : "Lintel shows them on its bar, so the dock leaves them out")
      : "While Lintel shows them on its bar. It isn’t now, so they’re in the dock", sw(`data-set="tray_too"`, s.tray_too, "Keep the icons in the dock too"))}
    <h3>PINNED APPS</h3>
    <div class="pins" id="pins"></div>
    ${setRow("Add an app", "From the Start menu; or drop a shortcut, program or folder on the dock", `<button class="btn" data-do="add">${picking ? "Done" : "Add…"}</button>`)}
    <div id="picker" hidden></div>
    <h3>LOOK</h3>
    ${hello.shared_look
      ? setRow("Theme, palette and glass", "Mullion Settings sets the look for every app", "")
      : setRow("Theme", "", seg("theme", [["auto", "Auto"], ["dark", "Dark"], ["light", "Light"]], s.theme))
        + setRow("Palette", esc(PALETTES.find(p => p[0] === s.palette)?.[1] ?? ""), `<div class="swatches">${PALETTES.map(([id, name, c]) => `<button class="swatch" data-palette="${id}" title="${name}" aria-label="${name}" aria-pressed="${s.palette === id}" style="background-color:${c}"></button>`).join("")}</div>`)
        + setRow("Glass", "The wallpaper, blurred, behind the dock", sw(`data-set="glass"`, s.glass, "Glass"))}
    <h3>GENERAL</h3>
    ${hello.mullion
      ? setRow("Start at sign-in", "Mullion starts Apron: see Mullion Settings", "")
      : setRow("Start at sign-in", "", sw(`data-do="autostart"`, hello.autostart, "Start at sign-in"))}
    ${setRow("Quit Apron", s.float ? "Until you start it again" : "The taskbar comes back until you start it again", `<button class="btn danger" data-do="quit">Quit</button>`)}
    <p class="about">Apron ${esc(hello.version)} · part of the Mullion suite, with Oriel, Lintel, Fanlight, Bay, Sash and Mitre<br>Settings: ${esc(hello.settings_path)}</p>`;
  drawPins();
  drawPicker();
}

// --- pinned apps ---------------------------------------------------------------------
// The dock's pinned apps, each with Remove; Add lists the Start menu's apps, found once.

let pinned = [], startApps = null, picking = false, query = "";

const appIcon = a => a.icon ? `<img src="${a.icon}" alt="">` : `<span class="initial">${esc((a.name || "?").slice(0, 1))}</span>`;

function drawPins() {
  const el = $("pins");
  if (!el) return;
  el.innerHTML = pinned.length
    ? pinned.map(p => `<div class="pin">${appIcon(p)}<span class="name">${esc(p.name)}</span><button class="btn small" data-unpin="${esc(p.key)}">Remove</button></div>`).join("")
    : `<p class="hint">Nothing pinned: the dock shows the apps that are running.</p>`;
}

function drawPicker() {
  const el = $("picker");
  if (!el) return;
  el.hidden = !picking;
  if (!picking) return;
  if (!$("find")) {
    el.innerHTML = `<input id="find" type="search" placeholder="Find an app" spellcheck="false" autocomplete="off"><div id="found"></div>`;
    const find = $("find");
    find.value = query;
    find.addEventListener("input", () => { query = find.value; drawFound(); });
    find.focus();
  }
  drawFound();
}

function drawFound() {
  const el = $("found");
  if (!el) return;
  if (!startApps) { el.innerHTML = `<p class="hint">Looking in the Start menu…</p>`; return; }
  const q = query.trim().toLowerCase();
  const shown = startApps.filter(a => !q || a.name.toLowerCase().includes(q));
  const keys = new Set(pinned.map(p => p.key));
  el.innerHTML = shown.length
    ? shown.map(a => `<div class="pin">${appIcon(a)}<span class="name">${esc(a.name)}</span>${keys.has(a.key)
      ? `<span class="in">In the dock</span>` : `<button class="btn small" data-add="${esc(a.launch)}">Add</button>`}</div>`).join("")
    : `<p class="hint">No app by that name in the Start menu.</p>`;
}

/** Which screen the dock is on, when there's more than one (or the one chosen is unplugged). */
function screenRow() {
  const all = hello.screens, s = settings;
  const missing = s.screen && !all.some(c => c.id === s.screen);
  if (all.length < 2 && !missing) return "";
  const sub = missing ? "The one chosen isn’t plugged in: it’s on the main screen until it is"
    : esc(all.find(c => c.id === s.screen)?.size ?? "");
  const control = all.length <= 3
    ? seg("screen", all.map(c => [c.id, esc(c.name)]), missing ? "" : s.screen)
    : `<select data-pick="screen">${all.map(c => `<option value="${esc(c.id)}"${c.id === s.screen ? " selected" : ""}>${esc(c.name)} · ${esc(c.size)}</option>`).join("")}</select>`;
  return setRow("Screen", sub, control);
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

setv.addEventListener("click", async e => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.dataset.set) return save({ [t.dataset.set]: t.getAttribute("aria-checked") !== "true" });
  if (t.dataset.screen !== undefined) return save({ screen: t.dataset.screen });
  if (t.dataset.edge) return save({ edge: t.dataset.edge });
  if (t.dataset.size) return save({ size: t.dataset.size });
  if (t.dataset.theme) return save({ theme: t.dataset.theme });
  if (t.dataset.palette) return save({ palette: t.dataset.palette });
  if (t.dataset.do === "get") return invoke("get_apps");
  if (t.dataset.unpin) return invoke("unpin", { key: t.dataset.unpin });
  if (t.dataset.add) return invoke("pin_paths", { paths: [t.dataset.add] });
  if (t.dataset.do === "add") {
    picking = !picking;
    t.textContent = picking ? "Done" : "Add…";
    if (picking && !startApps) invoke("start_menu_apps").then(a => { startApps = a; drawFound(); });
    return drawPicker();
  }
  if (t.dataset.do === "autostart") {
    hello.autostart = await invoke("set_autostart", { enabled: !hello.autostart }).catch(() => hello.autostart);
    return draw();
  }
  if (t.dataset.do === "quit") invoke("quit");
});

setv.addEventListener("change", e => {
  if (e.target.dataset.pick === "screen") save({ screen: e.target.value });
});

// Mullion's look for every app changed, or went on or off.
T.event.listen("look", e => { if (hello) hello.shared_look = e.payload; });
// The dock's menu changed something (Float and auto-hide), or the look changed.
T.event.listen("settings", e => { if (!hello) return; settings = e.payload; applyTheme(); draw(); });
// Lintel started or stopped showing the notification area's icons.
T.event.listen("lintel_tray", e => { if (!hello) return; hello.lintel_tray = e.payload; draw(); });
// Apps pinned or unpinned, here or on the dock.
T.event.listen("items", e => { pinned = e.payload.filter(i => i.pinned); drawPins(); drawFound(); });

invoke("hello").then(h => {
  hello = h;
  settings = h.settings;
  pinned = h.items.filter(i => i.pinned);
  $("logo").innerHTML = `<img src="${h.icon}" alt="">`;
  applyTheme();
  draw();
  invoke("settings_ready");
});
}
