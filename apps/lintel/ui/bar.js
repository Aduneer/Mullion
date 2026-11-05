"use strict";
{
const { invoke, listen, applyTheme, esc, icons, player, duration } = Lintel;
const $ = sel => document.querySelector(sel);
const items = Object.fromEntries([...document.querySelectorAll(".item")].map(b => [b.dataset.menu, b]));

let settings, info, sample, front;
/** The running apps, for the icons in the middle of the bar, and those this bar shows. */
let running = [], shown = [];
/** This bar's screen, and how many bars there are: with several, each shows its own screen's apps. */
let place = { monitor: 0, bars: 1 };
/** The menu on screen, if any: its bar item gets the pressed look, and hovering another item switches to it. */
let openMenu = null;
/** The notification area's icons, from Apron, and whether Apron is running to give them. */
let tray = [], trayHost = false;

items.system.innerHTML = icons.logo();

// --- menus -------------------------------------------------------------------------

function open(id, toggle, el = items[id]) {
  const { left, top, width, height } = el.getBoundingClientRect();
  invoke("open_menu", { id, item: { left, top, width, height }, toggle });
}

for (const [id, item] of Object.entries(items)) {
  item.addEventListener("pointerdown", e => {
    if (e.button === 0) open(id, true);
  });
  // As on a Mac: with a menu open, moving along the bar opens the menu under the pointer.
  item.addEventListener("pointerenter", () => {
    if (openMenu && openMenu !== id) open(id, false);
  });
}
// A click on an empty part of the bar closes the menu.
$("#bar").addEventListener("pointerdown", e => {
  if (openMenu && !e.target.closest(".item")) invoke("close_menu", { restore: true });
});

const menuItems = () => [
  ...Object.entries(items),
  ...[...dock.children].map(b => [`dock:${b.dataset.key}`, b]),
  ...[...trayEl.children].map(b => [`tray:${b.dataset.id}`, b]),
];

// The open menu's item becomes a tab in the menu's own color, running into the menu below it (or
// beside it), so the two read as one piece. Moving to another item slides the tab along with it.
const tab = document.createElement("div");
tab.id = "tab";
$("#bar").prepend(tab);
function showTab(el) {
  const bar = $("#bar").getBoundingClientRect(), r = el.getBoundingClientRect();
  const fresh = !tab.classList.contains("on");
  tab.classList.toggle("instant", fresh);
  Object.assign(tab.style, settings.edge === "left" || settings.edge === "right"
    ? { top: `${r.top - bar.top}px`, height: `${r.height}px`, left: "", width: "" }
    : { left: `${r.left - bar.left}px`, width: `${r.width}px`, top: "", height: "" });
  if (fresh) void tab.offsetWidth; // lay it out where it starts, so it fades in there, not slides
  tab.classList.remove("instant");
  tab.classList.add("on");
}

/** Keep the tab on its item as the bar changes around it (a wider number, an app opening). */
function retab() {
  const el = openMenu && menuItems().find(([k]) => k === openMenu)?.[1];
  if (el) showTab(el);
}

listen("menu-opened", id => {
  openMenu = id;
  for (const [k, item] of menuItems()) {
    item.classList.toggle("open", k === id);
    if (k === id) showTab(item);
  }
});
listen("menu-closed", () => {
  openMenu = null;
  tab.classList.remove("on");
  for (const [, item] of menuItems()) item.classList.remove("open");
});

// The scroll wheel on the speaker changes the volume, two steps at a time.
items.volume.addEventListener("wheel", e => {
  const a = sample?.audio;
  if (!a) return;
  const volume = Math.max(0, Math.min(100, a.volume + (e.deltaY < 0 ? 2 : -2)));
  a.volume = volume; // show it now rather than at the next tick
  renderVolume();
  invoke("set_volume", { volume });
}, { passive: true });

// Middle click on the track plays or pauses.
items.media.addEventListener("auxclick", e => {
  if (e.button === 1) invoke("media_control", { action: "toggle" });
});

// --- running apps -------------------------------------------------------------------
// As on the taskbar: a click brings the app forward, or minimizes it if it's already in front;
// an app in front with several windows lists them instead. Right click for its menu, middle click
// for a new window.

const dock = $("#apps");
const appOf = el => shown.find(a => a.key === el.dataset.key);

async function refreshPlace() {
  place = (await invoke("bar_info").catch(() => null)) ?? place;
  renderApps();
}

dock.addEventListener("pointerdown", e => {
  const el = e.target.closest(".dock-app");
  const a = el && appOf(el);
  if (!a) return;
  const id = `dock:${a.key}`;
  if (e.button === 2) return open(id, true, el);
  if (e.button !== 0) return;
  const inFront = a.windows.find(w => w.front);
  if (openMenu === id || (inFront && a.windows.length > 1)) return open(id, true, el);
  if (openMenu) invoke("close_menu", { restore: false });
  const w = inFront ?? a.windows[0];
  invoke("window_action", { action: inFront ? "minimize" : "focus", hwnd: w.hwnd, pid: w.pid }).catch(err => console.warn(err));
});
dock.addEventListener("auxclick", e => {
  const el = e.target.closest(".dock-app");
  if (e.button === 1 && el) invoke("launch_app", { key: el.dataset.key }).catch(err => console.warn(err));
});
dock.addEventListener("pointerover", e => {
  const el = e.target.closest(".dock-app");
  if (el && openMenu && openMenu !== `dock:${el.dataset.key}`) open(`dock:${el.dataset.key}`, false, el);
});

function renderApps() {
  const own = settings.apps_per_screen && place.bars > 1;
  shown = own
    ? running.map(a => ({ ...a, windows: a.windows.filter(w => w.monitor === place.monitor) })).filter(a => a.windows.length)
    : running;
  dock.hidden = !settings.apps || !shown.length;
  const old = new Map([...dock.children].map(b => [b.dataset.key, b]));
  shown.forEach((a, i) => {
    let b = old.get(a.key);
    old.delete(a.key);
    if (!b) {
      b = document.createElement("button");
      b.className = "item dock-app";
      b.dataset.key = a.key;
      b.innerHTML = `<img alt=""><span class="initial"></span><span class="dots"></span>`;
    }
    const img = b.querySelector("img");
    if (a.icon && b.dataset.icon !== a.icon.slice(-32)) {
      img.src = a.icon;
      b.dataset.icon = a.icon.slice(-32);
    }
    img.hidden = !a.icon;
    b.querySelector(".initial").textContent = a.icon ? "" : a.name.slice(0, 1);
    const n = a.windows.length;
    b.title = n > 1 ? `${a.name} (${n} windows)` : a.name;
    b.classList.toggle("front", a.windows.some(w => w.front));
    b.classList.toggle("minimized", a.windows.every(w => w.minimized));
    b.classList.toggle("open", openMenu === `dock:${a.key}`);
    b.querySelector(".dots").innerHTML = "<i></i>".repeat(Math.min(n, 3));
    if (dock.children[i] !== b) dock.insertBefore(b, dock.children[i] ?? null);
  });
  for (const b of old.values()) b.remove();
}

// --- the notification area ----------------------------------------------------------
// Other programs' icons (Steam, OneDrive…), which Apron hosts and passes on. A click goes back to
// Apron, which hands it to the program as Explorer would, so its own window or menu opens. The
// ones Windows keeps hidden are in a menu under the chevron.

const trayEl = $("#tray");
const trayIconOf = el => tray.find(t => t.id === el.dataset.id);
/** A picture in one colour, white or black for Windows' own taskbar, takes the bar's ink. */
const trayPicture = t => t.glyph ? `<i style="--src:url('${t.icon}')"></i>` : `<img alt="" src="${t.icon}">`;
const box = el => { const { left, top, width, height } = el.getBoundingClientRect(); return { left, top, width, height }; };

function renderTray() {
  const on = settings.tray && trayHost;
  const shown = on ? tray.filter(t => t.promoted && t.icon) : [];
  const hidden = on ? tray.filter(t => !t.promoted) : [];
  trayEl.hidden = !shown.length;
  items.tray.hidden = !hidden.length;
  // It points the way its menu opens: off the bar, into the screen.
  const towards = { top: "down", bottom: "up", left: "right", right: "left" }[settings.edge];
  if (items.tray.dataset.dir !== towards) {
    items.tray.innerHTML = icons.chevron(towards);
    items.tray.dataset.dir = towards;
  }
  items.tray.title = `${hidden.length} hidden icon${hidden.length === 1 ? "" : "s"}`;
  const old = new Map([...trayEl.children].map(b => [b.dataset.id, b]));
  shown.forEach((t, i) => {
    let b = old.get(t.id);
    old.delete(t.id);
    if (!b) {
      b = document.createElement("button");
      b.className = "item tray-icon";
      b.dataset.id = t.id;
    }
    const sig = `${t.glyph}|${t.icon.slice(-40)}`;
    if (b.dataset.sig !== sig) {
      b.innerHTML = trayPicture(t);
      b.dataset.sig = sig;
    }
    b.classList.toggle("glyph", t.glyph);
    // The standard tooltip; a program that draws its own is told the pointer's there instead.
    if (t.tooltip && t.tip) b.title = t.tip;
    else b.removeAttribute("title");
    b.classList.toggle("open", openMenu === `tray:${t.id}`);
    if (trayEl.children[i] !== b) trayEl.insertBefore(b, trayEl.children[i] ?? null);
  });
  for (const b of old.values()) b.remove();
}

async function trayClick(el, kind) {
  const t = trayIconOf(el);
  if (!t) return;
  if (openMenu) invoke("close_menu", { restore: false });
  // Its program runs as administrator, out of reach: a menu says so, and offers to open it.
  const out = await invoke("tray_click", { id: t.id, kind, item: box(el) }).catch(err => (console.warn(err), null));
  if (out) open(`tray:${t.id}`, true, el);
}

trayEl.addEventListener("click", e => {
  const el = e.target.closest(".tray-icon");
  if (el && e.detail === 1) trayClick(el, "left");
});
trayEl.addEventListener("dblclick", e => {
  const el = e.target.closest(".tray-icon");
  if (el) trayClick(el, "double");
});
trayEl.addEventListener("contextmenu", e => {
  const el = e.target.closest(".tray-icon");
  if (el) trayClick(el, "right");
});
trayEl.addEventListener("pointerover", e => {
  const el = e.target.closest(".tray-icon");
  if (el && !el.contains(e.relatedTarget)) invoke("tray_hover", { id: el.dataset.id, on: true, item: box(el) });
});
trayEl.addEventListener("pointerout", e => {
  const el = e.target.closest(".tray-icon");
  if (el && !el.contains(e.relatedTarget) && !el.title) invoke("tray_hover", { id: el.dataset.id, on: false });
});

// --- rendering ---------------------------------------------------------------------

function renderFront() {
  $("#app-name").textContent = front?.name || "Desktop";
}

const vertical = () => settings.edge === "left" || settings.edge === "right";

function renderClock() {
  const now = new Date();
  const h12 = settings.clock === "auto" ? !info.clock24 : settings.clock === "12";
  const opts = { hour: "numeric", minute: "2-digit", second: settings.seconds ? "2-digit" : undefined, hour12: h12 };
  if (vertical()) {
    // Standing on its side, the bar is too narrow for "5:42 PM" or "Sat 24 Oct" on one line.
    // Hours and minutes on one line; seconds and AM/PM, if shown, small underneath.
    const parts = new Intl.DateTimeFormat(undefined, opts).formatToParts(now);
    const part = type => parts.find(p => p.type === type)?.value;
    const sep = parts.find(p => p.type === "literal" && p.value.trim())?.value.trim() ?? ":";
    const under = [part("second"), part("dayPeriod")].filter(Boolean).join(" ");
    $("#time").innerHTML = esc(`${part("hour")}${sep}${part("minute")}`) + (under ? `<small>${esc(under)}</small>` : "");
    $("#date").innerHTML = `<small>${esc(now.toLocaleDateString(undefined, { weekday: "short" }))}</small>${now.getDate()}`;
  } else {
    $("#time").textContent = now.toLocaleTimeString(undefined, opts);
    $("#date").textContent = now.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
  }
  $("#date").hidden = !settings.date;
}

function level(pct, warn = 80, crit = 92) {
  return pct >= crit ? "crit" : pct >= warn ? "warn" : "";
}

function renderStats() {
  const s = sample?.stats;
  if (!s) return;
  const cpu = $("#cpu"), mem = $("#mem");
  cpu.textContent = `${Math.round(s.cpu)}%`;
  cpu.className = level(s.cpu);
  mem.textContent = `${Math.round(s.mem.pct)}%`;
  mem.className = level(s.mem.pct);
  items.stats.title = `CPU ${s.cpu.toFixed(1)}%
Memory ${s.mem.pct.toFixed(1)}% (${Lintel.bytes(s.mem.used)} of ${Lintel.bytes(s.mem.total, 0)})`;
}

function renderVolume() {
  const a = sample?.audio;
  const item = items.volume;
  item.hidden = !settings.volume || !a;
  if (!a) return;
  item.innerHTML = icons.speaker(a.volume, a.muted || a.volume === 0);
  item.title = `${a.device}: ${a.muted ? "muted" : `${a.volume}%`}`;
}

function renderNetwork() {
  const l = sample?.stats.link;
  const item = items.network;
  item.hidden = !settings.network || !l;
  if (!l) return;
  item.innerHTML = l.kind === "wifi" ? icons.wifi(l.signal ?? 100) : l.kind === "wired" ? icons.wired() : icons.offline();
  item.title = l.kind === "wifi" ? `Wi-Fi${l.ssid ? `: ${l.ssid}` : ""}${l.internet ? "" : " (no internet)"}` : l.kind === "wired" ? `Wired${l.internet ? "" : " (no internet)"}` : "Not connected";
}

function renderBluetooth() {
  const on = sample?.radios?.bluetooth;
  const item = items.bluetooth;
  item.hidden = !settings.bluetooth || on == null;
  if (on == null) return;
  item.innerHTML = icons.bluetooth(on);
  item.title = `Bluetooth: ${on ? "on" : "off"}`;
}

function renderControl() {
  items.control.hidden = !settings.control;
  if (!items.control.firstChild) items.control.innerHTML = icons.control();
}

function renderBattery() {
  const b = sample?.stats.battery;
  const item = items.battery;
  item.hidden = !settings.battery || !b;
  if (!b) return;
  item.querySelector(".glyph").innerHTML = icons.battery(b.pct, b.plugged);
  item.querySelector(".text").textContent = `${b.pct}%`;
  item.title = b.plugged ? (b.pct >= 100 ? "Fully charged" : "Charging") : b.secs_left ? `${duration(b.secs_left)} left` : "On battery";
}

let lastTrack = "";
function renderMedia() {
  const m = sample?.media;
  const item = items.media;
  const has = settings.media && m && m.title;
  item.hidden = !has;
  if (!has) return;
  const track = [m.title, m.artist].filter(Boolean).join(" · ");
  if (track !== lastTrack) {
    item.querySelector(".text").textContent = track;
    lastTrack = track;
  }
  item.classList.toggle("paused", !m.playing);
  item.querySelector(".glyph").innerHTML = m.playing ? icons.note() : icons.pause();
  item.title = `${track}${m.album ? ` — ${m.album}` : ""} (${player(m.app)})`;
}

function renderAll() {
  document.body.dataset.edge = settings.edge;
  document.body.style.setProperty("--h", `${settings.height}px`);
  document.body.classList.toggle("vertical", vertical());
  items.stats.hidden = !settings.stats;
  renderFront();
  renderApps();
  renderTray();
  renderClock();
  renderStats();
  renderMedia();
  renderVolume();
  renderNetwork();
  renderBluetooth();
  renderControl();
  renderBattery();
}

// --- glass -------------------------------------------------------------------------
// The bar keeps its strip of the screen to itself, so only the wallpaper is ever behind it: it
// shows that part of the wallpaper, blurred (common.js), with the palette's tint over it (bar.css).

const backdrop = $("#backdrop");

async function refreshBackdrop() {
  backdrop.hidden = !settings.glass;
  if (settings.glass) await Lintel.backdrop(backdrop);
}

let backdropTimer = 0;
const backdropSoon = () => { clearTimeout(backdropTimer); backdropTimer = setTimeout(refreshBackdrop, 150); };
window.addEventListener("resize", () => { backdropSoon(); retab(); });
listen("backdrop", () => { backdropSoon(); refreshPlace(); });

// --- data ----------------------------------------------------------------------------

listen("sample", s => {
  sample = s;
  renderStats();
  renderMedia();
  renderVolume();
  renderNetwork();
  renderBluetooth();
  renderBattery();
  retab();
});
listen("front", f => {
  front = f;
  renderFront();
  retab();
});
listen("apps", a => {
  running = a;
  renderApps();
  retab();
});
listen("tray", t => {
  tray = t;
  if (settings) renderTray();
  retab();
});
listen("tray-host", on => {
  trayHost = on;
  if (settings) renderTray();
  retab();
});
listen("settings", s => {
  settings = s;
  applyTheme(s);
  renderAll();
  tick();
  backdropSoon();
  retab();
});

// The clock ticks on the minute (or second), whatever the samples do.
let timer = 0;
function tick() {
  clearTimeout(timer);
  renderClock();
  retab();
  const ms = settings.seconds ? 1000 - (Date.now() % 1000) : 60000 - (Date.now() % 60000);
  timer = setTimeout(tick, ms + 5);
}

invoke("hello").then(h => {
  ({ settings, sample, front } = h);
  running = h.apps ?? [];
  tray = h.tray ?? [];
  trayHost = !!h.tray_host;
  info = h;
  applyTheme(settings);
  renderAll();
  tick();
  refreshBackdrop();
  refreshPlace();
});
}
