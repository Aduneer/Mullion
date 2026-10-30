"use strict";
// Mitre's settings: what each corner of each screen does, how long the pointer waits there, its
// look, and its sign-in entry. Each change is saved as it's made and works at once.
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
// The corners, in the order Mitre keeps them.
const CORNERS = [["tl", "Top left"], ["tr", "Top right"], ["bl", "Bottom left"], ["br", "Bottom right"]];
const NOTHING = "nothing";
// On a button that opens a page in the browser, so it never does that unannounced.
const OUT = `<svg class="ic out" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 5h5v5M19 5l-8 8M17 14v4a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1h4"/></svg>`;

let hello = null, settings = null;
// The screen whose corners are shown: its key.
let picked = null;

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

const corners = key => settings.corners[key] ?? [NOTHING, NOTHING, NOTHING, NOTHING];
const action = id => hello.actions.find(a => a.id === id);

// The screens, small, where Windows has them: a button each.
function drawMap() {
  const ss = hello.screens;
  if (ss.length < 2) return "";
  const left = Math.min(...ss.map(s => s.rect[0])), top = Math.min(...ss.map(s => s.rect[1]));
  const right = Math.max(...ss.map(s => s.rect[0] + s.rect[2])), bottom = Math.max(...ss.map(s => s.rect[1] + s.rect[3]));
  const k = Math.min(260 / (right - left), 56 / (bottom - top));
  const gap = 3;
  return `<div class="map" style="width:${Math.round((right - left) * k)}px;height:${Math.round((bottom - top) * k)}px">${ss.map(s =>
    `<button data-screen="${esc(s.key)}" aria-pressed="${s.key === picked}" title="${esc(s.name)}${s.main ? ", the main screen" : ""}"
      style="left:${Math.round((s.rect[0] - left) * k) + gap / 2}px;top:${Math.round((s.rect[1] - top) * k) + gap / 2}px;width:${Math.round(s.rect[2] * k) - gap}px;height:${Math.round(s.rect[3] * k) - gap}px">${esc(s.name.replace("Screen ", ""))}</button>`).join("")}</div>`;
}

// A corner's menu: every action, those whose app isn't installed greyed out.
function menu(s, i) {
  const [cls, name] = CORNERS[i];
  const now = corners(s.key)[i];
  const options = hello.actions.map(a => `<option value="${esc(a.id)}" ${a.id === now ? "selected" : ""} ${a.available || a.id === now ? "" : "disabled"}>${
    esc(a.id === NOTHING ? "—" : a.name)}${a.available || a.id === now ? "" : " (not installed)"}</option>`).join("");
  const notes = [];
  if (now !== NOTHING && !action(now)?.available) notes.push(`${esc(action(now)?.app ?? "Its app")} isn’t installed, so this does nothing`);
  const side = `${cls[1] === "r" ? " right" : ""}${cls[0] === "b" ? " bottom" : ""}`;
  return `<div class="menu${side}" style="grid-column:${cls[1] === "l" ? 1 : 3};grid-row:${cls[0] === "t" ? 1 : 2}">
    <select class="${now === NOTHING ? "none" : ""}" data-corner="${i}" aria-label="${esc(name)}">${options}</select>
    ${notes.map(n => `<small>${n}</small>`).join("")}</div>`;
}

function drawCorners() {
  const s = hello.screens.find(s => s.key === picked) ?? hello.screens[0];
  if (!s) return `<p class="hint">Windows lists no screens just now.</p>`;
  picked = s.key;
  const ratio = s.rect[3] / s.rect[2];
  const set = corners(s.key);
  const inner = CORNERS.filter((_, i) => !s.hard[i]).map(([, name]) => name.toLowerCase());
  const innerNote = !inner.length ? "" : `<p class="hint" style="padding:10px 0 0">${inner.length === 4 ? "Every corner" : inner.join(inner.length > 2 ? ", " : " and ").replace(/^./, c => c.toUpperCase())}
    ${inner.length > 1 ? "meet" : "meets"} another screen, so the pointer slides through to it: ${inner.length > 1 ? "they’re" : "it’s"} hard to hit.</p>`;
  return `${drawMap()}<div class="corners">
    ${CORNERS.map((_, i) => menu(s, i)).join("")}
    <div class="screen" style="aspect-ratio:${s.rect[2]} / ${s.rect[3]};${ratio > 0.75 ? "max-width:180px;justify-self:center" : ""}">
      ${CORNERS.map(([cls], i) => `<span class="glow ${cls}${set[i] !== NOTHING && action(set[i])?.available ? " on" : ""}"></span>`).join("")}
      <div class="label"><div><b>${esc(s.name)}</b>${s.main ? "Main screen · " : ""}${s.rect[2]} × ${s.rect[3]}${s.scale !== 100 ? ` · ${s.scale}%` : ""}</div></div>
    </div></div>${innerNote}`;
}

function draw() {
  const s = settings;
  const missing = [...new Set(hello.actions.filter(a => !a.available && a.app).map(a => a.app))];
  setv.innerHTML = `
    <h3>CORNERS</h3>
    <p class="hint">Push the pointer into a corner and wait a moment. Pushed again, Bay or Fanlight closes again.${hello.screens.length > 1 ? " Pick a screen to set its corners." : ""}</p>
    ${drawCorners()}
    ${missing.length ? setRow(`${missing.join(" and ")} ${missing.length > 1 ? "aren’t" : "isn’t"} installed`, "Mitre can open them from a corner once they are",
      `<button class="btn" data-do="get" title="Opens GitHub in your browser">Get ${missing.length > 1 ? "them" : "it"}${OUT}</button>`) : ""}
    <h3>HOW IT FIRES</h3>
    ${setRow("Wait in the corner", "Longer misses more of the pointer passing through", seg("dwell", hello.dwells.map(d => [d, `${d / 1000} s`]), s.dwell))}
    ${setRow("Glow", "Grows in the corner while the pointer waits there", sw(`data-set="glow"`, s.glow, "Glow"))}
    <p class="hint" style="padding-top:8px">Nothing fires while a mouse button is held (dragging into a corner) or a full-screen app or game is in front on that screen.</p>
    <h3>LOOK</h3>
    ${hello.shared_look
      ? setRow("Theme and palette", "Mullion Settings sets the look for every app", "")
      : setRow("Theme", "", seg("theme", [["auto", "Auto"], ["dark", "Dark"], ["light", "Light"]], s.theme))
        + setRow("Palette", `${esc(PALETTES.find(p => p[0] === s.palette)?.[1] ?? "")}, and the glow’s colour`, `<div class="swatches">${PALETTES.map(([id, name, c]) => `<button class="swatch" data-palette="${id}" title="${name}" aria-label="${name}" aria-pressed="${s.palette === id}" style="background-color:${c}"></button>`).join("")}</div>`)}
    <h3>GENERAL</h3>
    ${hello.mullion
      ? setRow("Start at sign-in", "Mullion starts Mitre: see Mullion Settings", "")
      : setRow("Start at sign-in", "", sw(`data-do="autostart"`, hello.autostart, "Start at sign-in"))}
    ${setRow("Quit Mitre", "Until you start it again", `<button class="btn danger" data-do="quit">Quit</button>`)}
    <p class="about">Mitre ${esc(hello.version)} · part of the Mullion suite, with Oriel, Lintel, Fanlight, Bay and Sash<br>Settings: ${esc(hello.settings_path)}</p>`;
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

setv.addEventListener("change", e => {
  const t = e.target.closest("select[data-corner]");
  if (!t) return;
  const set = [...corners(picked)];
  set[Number(t.dataset.corner)] = t.value;
  save({ corners: { ...settings.corners, [picked]: set } });
});

setv.addEventListener("click", async e => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.dataset.screen) {
    picked = t.dataset.screen;
    return draw();
  }
  if (t.dataset.set) return save({ [t.dataset.set]: t.getAttribute("aria-checked") !== "true" });
  if (t.dataset.dwell) return save({ dwell: Number(t.dataset.dwell) });
  if (t.dataset.theme) return save({ theme: t.dataset.theme });
  if (t.dataset.palette) return save({ palette: t.dataset.palette });
  if (t.dataset.do === "get") return invoke("get_apps");
  if (t.dataset.do === "autostart") {
    hello.autostart = await invoke("set_autostart", { enabled: !hello.autostart }).catch(() => hello.autostart);
    return draw();
  }
  if (t.dataset.do === "quit") invoke("quit");
});

// Mullion's look for every app changed, or went on or off.
T.event.listen("look", e => { if (hello) hello.shared_look = e.payload; });
T.event.listen("settings", e => { if (!hello) return; settings = e.payload; applyTheme(); draw(); });
// A screen came or went, or moved.
T.event.listen("screens", e => { if (!hello) return; hello.screens = e.payload; draw(); });

invoke("hello").then(h => {
  hello = h;
  settings = h.settings;
  picked = (h.screens.find(s => s.main) ?? h.screens[0])?.key ?? null;
  $("logo").innerHTML = `<img src="${h.icon}" alt="">`;
  applyTheme();
  draw();
  invoke("settings_ready");
});
}
