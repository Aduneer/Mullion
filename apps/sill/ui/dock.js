"use strict";
// The dock: Fanlight's apps button, the pinned and running apps (a dot per window), and the
// notification area's icons. A click brings an app forward, or minimizes it if it's in front, or
// opens it; a right click is its menu (its windows, a new one, Pin, Quit); a middle click opens a
// new window. Pinned apps can be dragged into a new order.
{
const T = window.__TAURI__;
const me = T.webviewWindow?.getCurrentWebviewWindow?.();
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const listen = (event, fn) => (me ?? T.event).listen(event, e => fn(e.payload));
const $ = id => document.getElementById(id);
const root = document.documentElement;
const shelfEl = $("shelf"), appsEl = $("apps"), trayEl = $("tray"), gridEl = $("grid");

// A dock, not a web page: no browser right-click menu, and none of its keys.
document.addEventListener("contextmenu", e => e.preventDefault());
document.addEventListener("keydown", e => {
  const k = e.key.toLowerCase();
  if (["f5", "f7", "f12", "browserback", "browserforward"].includes(k)
    || (e.ctrlKey && ["r", "f", "g", "p", "u", "j", "+", "-", "=", "0"].includes(k))
    || (e.ctrlKey && e.shiftKey && ["i", "c"].includes(k))
    || (e.altKey && ["arrowleft", "arrowright"].includes(k))) e.preventDefault();
});
document.addEventListener("wheel", e => { if (e.ctrlKey) e.preventDefault(); }, { passive: false });

const SIZES = { small: 32, medium: 40, large: 52 };
const svg = body => `<svg class="ic" viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;
const GRID = svg(`<rect x="4" y="4" width="6.5" height="6.5" rx="1.8"/><rect x="13.5" y="4" width="6.5" height="6.5" rx="1.8"/><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.8"/><rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.8"/>`);
const MORE = svg(`<path d="M7 14.5l5-5 5 5"/>`);

let settings = null, items = [], tray = [], fanlight = false;

// --- look ----------------------------------------------------------------------------

const dark = matchMedia("(prefers-color-scheme: dark)");
function applyLook() {
  root.dataset.palette = settings.palette;
  root.dataset.mode = settings.theme === "auto" ? (dark.matches ? "dark" : "light") : settings.theme;
  root.dataset.glass = settings.glass ? "on" : "off";
  root.dataset.float = settings.float ? "on" : "off";
  root.dataset.edge = settings.edge;
  const icon = SIZES[settings.size] ?? 40;
  document.body.style.setProperty("--icon", `${icon}px`);
  document.body.style.setProperty("--tray", `${icon >= 52 ? 20 : 16}px`);
  document.body.style.setProperty("--rows", icon >= 40 ? 2 : 1);
  if (settings.float) shelfEl.classList.add("out");
  else shelfEl.classList.remove("out");
  backdropSoon();
}
dark.addEventListener("change", () => settings && applyLook());

// --- glass ---------------------------------------------------------------------------
// In the taskbar's place nothing is behind the dock but the wallpaper, so with glass on it paints
// that, blurred, as the shelf's background (as Lintel's bar does: Windows draws a window without the focus,
// which the dock never has, flat gray). Floating, the shelf is just see-through.

let wallKey = null, wallImage = null, painted = null;

function paintWallpaper(g, w, img) {
  const [mx, my, mw, mh] = w.position === "span" ? w.desktop : w.monitor;
  const iw = img.naturalWidth, ih = img.naturalHeight;
  if (w.position === "stretch") return g.drawImage(img, mx, my, mw, mh);
  if (w.position === "center") return g.drawImage(img, mx + (mw - iw) / 2, my + (mh - ih) / 2);
  if (w.position === "tile") {
    g.save();
    g.translate(mx, my);
    g.fillStyle = g.createPattern(img, "repeat");
    g.fillRect(0, 0, mw, mh);
    return g.restore();
  }
  const scale = w.position === "fit" ? Math.min(mw / iw, mh / ih) : Math.max(mw / iw, mh / ih);
  g.drawImage(img, mx + (mw - iw * scale) / 2, my + (mh - ih * scale) / 2, iw * scale, ih * scale);
}

async function refreshBackdrop() {
  const canvas = $("backdrop");
  const on = settings?.glass && !settings.float;
  canvas.hidden = !on;
  if (!on) { painted = null; return; }
  const w = await invoke("wallpaper").catch(() => null);
  if (!w) return;
  // The canvas is the shelf's background: paint the part of the wallpaper the shelf covers.
  const d = devicePixelRatio, r = shelfEl.getBoundingClientRect();
  const [sx, sy, sw, sh] = [w.bar[0] + r.left * d, w.bar[1] + r.top * d, r.width * d, r.height * d].map(Math.round);
  const key = `${w.key}|${sx},${sy},${sw},${sh}`;
  if (key === painted) return;
  painted = key;
  if (w.key !== wallKey) {
    wallKey = w.key;
    const url = await invoke("wallpaper_image").catch(() => null);
    wallImage = url ? await new Promise(ok => {
      const img = new Image();
      img.onload = () => ok(img);
      img.onerror = () => ok(null);
      img.src = url;
    }) : null;
  }
  const pad = Math.ceil(60 * d);
  const src = new OffscreenCanvas(sw + 2 * pad, sh + 2 * pad);
  const g = src.getContext("2d");
  g.fillStyle = w.color;
  g.fillRect(0, 0, src.width, src.height);
  g.translate(pad - sx, pad - sy);
  if (wallImage) paintWallpaper(g, w, wallImage);
  canvas.width = sw;
  canvas.height = sh;
  const out = canvas.getContext("2d");
  out.filter = `blur(${Math.round(20 * d)}px) saturate(1.6)`;
  out.drawImage(src, -pad, -pad);
}

let backdropTimer = 0;
function backdropSoon() {
  clearTimeout(backdropTimer);
  backdropTimer = setTimeout(refreshBackdrop, 120);
}
// The wallpaper may change while the dock is up; this only repaints when it has.
setInterval(() => { if (settings?.glass && !settings.float) refreshBackdrop(); }, 15000);

// --- the shelf's size ------------------------------------------------------------------
// Floating, the window is just the shelf: tell Sill how big it is whenever that changes.

let lastShelf = "";
new ResizeObserver(() => {
  const size = `${shelfEl.offsetWidth}x${shelfEl.offsetHeight}`;
  if (size === lastShelf) return;
  lastShelf = size;
  invoke("shelf", { width: shelfEl.offsetWidth, height: shelfEl.offsetHeight });
  if (settings) backdropSoon();
}).observe(shelfEl);
addEventListener("resize", () => { if (settings) backdropSoon(); });

// --- apps ----------------------------------------------------------------------------

/** Where a menu opens from for `el`: off the shelf, towards the screen. */
function anchor(el) {
  const r = el.getBoundingClientRect(), s = shelfEl.getBoundingClientRect();
  if (settings.edge === "left") return { x: s.right + 4, y: r.top + r.height / 2 };
  if (settings.edge === "right") return { x: s.left - 4, y: r.top + r.height / 2 };
  return { x: r.left + r.width / 2, y: s.top - 4 };
}

const itemOf = el => items.find(a => a.key === el.dataset.key);
const launching = new Map();

function renderApps() {
  const old = new Map([...appsEl.children].map(b => [b.dataset.key, b]));
  items.forEach((a, i) => {
    let b = old.get(a.key);
    old.delete(a.key);
    if (!b) {
      b = document.createElement("button");
      b.className = "tile app";
      b.dataset.key = a.key;
      b.innerHTML = `<img alt=""><span class="initial"></span><span class="dots"></span>`;
    }
    const img = b.querySelector("img");
    const sig = a.icon ? a.icon.slice(-40) : "";
    if (a.icon && b.dataset.icon !== sig) {
      img.src = a.icon;
      b.dataset.icon = sig;
    }
    img.hidden = !a.icon;
    b.querySelector(".initial").textContent = a.icon ? "" : (a.name || "?").slice(0, 1);
    const n = a.windows.length;
    b.title = n > 1 ? `${a.name} (${n} windows)` : a.name;
    b.classList.toggle("pinned", a.pinned);
    b.classList.toggle("front", a.windows.some(w => w.front));
    b.classList.toggle("minimized", n > 0 && a.windows.every(w => w.minimized));
    b.querySelector(".dots").innerHTML = "<i></i>".repeat(Math.min(n, 4));
    // An app that was opened from here has its window now: stop bouncing.
    if (n && launching.has(a.key)) {
      clearTimeout(launching.get(a.key));
      launching.delete(a.key);
      b.classList.remove("bounce");
    }
    if (appsEl.children[i] !== b) appsEl.insertBefore(b, appsEl.children[i] ?? null);
  });
  for (const b of old.values()) b.remove();
}

function bounce(b, key) {
  b.classList.remove("bounce");
  void b.offsetWidth;
  b.classList.add("bounce");
  clearTimeout(launching.get(key));
  launching.set(key, setTimeout(() => { b.classList.remove("bounce"); launching.delete(key); }, 2200));
}

// Dragging a pinned app among the others puts it there; a short move is still a click.
let drag = null;
appsEl.addEventListener("pointerdown", e => {
  const b = e.target.closest(".tile.app");
  if (!b || e.button !== 0) return;
  drag = { b, x: e.clientX, y: e.clientY, moved: false, id: e.pointerId };
});
addEventListener("pointermove", e => {
  if (!drag || e.pointerId !== drag.id) return;
  if (!drag.moved) {
    if (Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 6 || !drag.b.classList.contains("pinned")) return;
    drag.moved = true;
    drag.b.classList.add("dragging");
    drag.b.setPointerCapture(e.pointerId);
  }
  const along = settings.edge === "bottom" ? e.clientX : e.clientY;
  const pinned = [...appsEl.querySelectorAll(".tile.pinned")];
  const before = pinned.find(p => {
    if (p === drag.b) return false;
    const r = p.getBoundingClientRect();
    return along < (settings.edge === "bottom" ? r.left + r.width / 2 : r.top + r.height / 2);
  });
  const last = pinned.filter(p => p !== drag.b).at(-1);
  if (before) appsEl.insertBefore(drag.b, before);
  else if (last) last.after(drag.b);
});
addEventListener("pointerup", e => {
  if (!drag || e.pointerId !== drag.id) return;
  const d = drag;
  drag = null;
  if (!d.moved) return click(d.b);
  d.b.classList.remove("dragging");
  const keys = [...appsEl.querySelectorAll(".tile.pinned")].map(p => p.dataset.key);
  items = [...keys.map(k => items.find(a => a.key === k)), ...items.filter(a => !a.pinned)];
  invoke("reorder", { keys });
});

async function click(b) {
  const a = itemOf(b);
  if (!a) return;
  const did = await invoke("activate", { key: a.key, ...anchor(b) }).catch(err => (console.warn(err), null));
  if (did === "launch") bounce(b, a.key);
}

appsEl.addEventListener("auxclick", e => {
  const b = e.target.closest(".tile.app");
  if (e.button !== 1 || !b) return;
  invoke("new_window", { key: b.dataset.key }).then(() => { if (!itemOf(b)?.windows.length) bounce(b, b.dataset.key); }).catch(err => console.warn(err));
});

shelfEl.addEventListener("contextmenu", e => {
  const app = e.target.closest(".tile.app"), icon = e.target.closest(".tray");
  if (icon) return;
  if (app) return invoke("app_menu", { key: app.dataset.key, ...anchor(app) }).catch(err => console.warn(err));
  invoke("dock_menu", anchor(e.target.closest("button") ?? shelfEl));
});

gridEl.innerHTML = GRID;
gridEl.addEventListener("click", () => { if (fanlight) invoke("open_grid").catch(err => console.warn(err)); });

function renderGrid() {
  gridEl.classList.toggle("off", !fanlight);
  gridEl.title = fanlight ? "Apps" : "Apps: Fanlight isn’t installed";
}

// --- the notification area -------------------------------------------------------------

function renderTray() {
  const shown = tray.filter(t => t.promoted && t.icon);
  const hidden = tray.filter(t => !t.promoted);
  $("tray-sep").hidden = !shown.length && !hidden.length;
  trayEl.innerHTML = (hidden.length ? `<button class="tray more" title="${hidden.length} more">${MORE}</button>` : "")
    + shown.map(t => `<button class="tray" data-id="${esc(t.id)}" title="${esc(t.tip)}"><img alt="" src="${t.icon}"></button>`).join("");
}
const esc = s => String(s ?? "").replace(/[&<>"']/g, c => `&#${c.charCodeAt(0)};`);

trayEl.addEventListener("click", e => {
  const b = e.target.closest(".tray");
  if (!b) return;
  if (b.classList.contains("more")) return invoke("tray_more", anchor(b)).catch(err => console.warn(err));
  if (e.detail === 1) invoke("tray_click", { id: b.dataset.id, kind: "left", ...anchor(b) }).catch(err => console.warn(err));
});
trayEl.addEventListener("dblclick", e => {
  const b = e.target.closest(".tray[data-id]");
  if (b) invoke("tray_click", { id: b.dataset.id, kind: "double", ...anchor(b) }).catch(err => console.warn(err));
});
trayEl.addEventListener("contextmenu", e => {
  const b = e.target.closest(".tray[data-id]");
  if (b) invoke("tray_click", { id: b.dataset.id, kind: "right", ...anchor(b) }).catch(err => console.warn(err));
});

// --- from Sill -----------------------------------------------------------------------

listen("items", i => { items = i; renderApps(); });
listen("tray", t => { tray = t; renderTray(); });
listen("settings", s => { settings = s; applyLook(); });
// Floating: the pointer called the dock, or left it.
listen("slide", show => shelfEl.classList.toggle("out", !show));

invoke("hello").then(h => {
  settings = h.settings;
  items = h.items;
  tray = h.tray;
  fanlight = h.fanlight;
  applyLook();
  renderGrid();
  renderApps();
  renderTray();
});
}
