"use strict";
{
const { invoke, listen, applyTheme, esc, icons, player, duration } = Lintel;
const $ = sel => document.querySelector(sel);
const items = Object.fromEntries([...document.querySelectorAll(".item")].map(b => [b.dataset.menu, b]));

let settings, info, sample, front;
/** The running apps, for the icons in the middle of the bar. */
let running = [];
/** The menu on screen, if any: its bar item gets the pressed look, and hovering another item switches to it. */
let openMenu = null;

items.system.innerHTML = icons.logo();

// --- menus -------------------------------------------------------------------------

function open(id, toggle, el = items[id]) {
  const { left, top } = el.getBoundingClientRect();
  invoke("open_menu", { id, item: { left, top }, toggle });
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

const menuItems = () => [...Object.entries(items), ...[...dock.children].map(b => [`dock:${b.dataset.key}`, b])];
listen("menu-opened", id => {
  openMenu = id;
  for (const [k, item] of menuItems()) item.classList.toggle("open", k === id);
});
listen("menu-closed", () => {
  openMenu = null;
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
const appOf = el => running.find(a => a.key === el.dataset.key);

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
  dock.hidden = !settings.apps || !running.length;
  const old = new Map([...dock.children].map(b => [b.dataset.key, b]));
  running.forEach((a, i) => {
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
  renderClock();
  renderStats();
  renderMedia();
  renderVolume();
  renderNetwork();
  renderBattery();
}

// --- glass -------------------------------------------------------------------------
// Windows only blurs behind windows that have the focus, and the bar never does. But the bar
// keeps its strip of the screen to itself, so only the wallpaper is ever behind it: it draws that
// part of the wallpaper, blurred, and the palette's tint goes over it (bar.css).

const backdrop = $("#backdrop");
let wall = null, wallKey = null, wallImage = null;

async function refreshBackdrop() {
  backdrop.hidden = !settings.glass;
  if (!settings.glass) return;
  const w = await invoke("wallpaper").catch(() => null);
  if (!w) return;
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
  wall = w;
  drawBackdrop();
}

/** The wallpaper as Windows lays it out, on a canvas whose origin is the screen's. */
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

function drawBackdrop() {
  const w = wall;
  if (!w) return;
  const [bx, by, bw, bh] = w.bar;
  // The blur reaches past the bar's edges, so draw a margin of the wallpaper around it too.
  const pad = Math.ceil(60 * devicePixelRatio);
  const src = new OffscreenCanvas(bw + 2 * pad, bh + 2 * pad);
  const g = src.getContext("2d");
  g.fillStyle = w.color;
  g.fillRect(0, 0, src.width, src.height);
  g.translate(pad - bx, pad - by);
  if (wallImage) paintWallpaper(g, w, wallImage);
  backdrop.width = bw;
  backdrop.height = bh;
  const out = backdrop.getContext("2d");
  out.filter = `blur(${Math.round(20 * devicePixelRatio)}px) saturate(1.6)`;
  out.drawImage(src, -pad, -pad);
}

let backdropTimer = 0;
const backdropSoon = () => { clearTimeout(backdropTimer); backdropTimer = setTimeout(refreshBackdrop, 150); };
window.addEventListener("resize", backdropSoon);
listen("backdrop", backdropSoon);

// --- data ----------------------------------------------------------------------------

listen("sample", s => {
  sample = s;
  renderStats();
  renderMedia();
  renderVolume();
  renderNetwork();
  renderBattery();
});
listen("front", f => {
  front = f;
  renderFront();
});
listen("apps", a => {
  running = a;
  renderApps();
});
listen("settings", s => {
  settings = s;
  applyTheme(s);
  renderAll();
  tick();
  backdropSoon();
});

// The clock ticks on the minute (or second), whatever the samples do.
let timer = 0;
function tick() {
  clearTimeout(timer);
  renderClock();
  const ms = settings.seconds ? 1000 - (Date.now() % 1000) : 60000 - (Date.now() % 60000);
  timer = setTimeout(tick, ms + 5);
}

invoke("hello").then(h => {
  ({ settings, sample, front } = h);
  running = h.apps ?? [];
  info = h;
  applyTheme(settings);
  renderAll();
  tick();
  refreshBackdrop();
});
}
