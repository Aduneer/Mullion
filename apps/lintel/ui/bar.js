"use strict";
{
const { invoke, listen, applyTheme, esc, icons, player, duration } = Lintel;
const $ = sel => document.querySelector(sel);
const items = Object.fromEntries([...document.querySelectorAll(".item")].map(b => [b.dataset.menu, b]));

let settings, info, sample, front;
/** The menu on screen, if any: its bar item gets the pressed look, and hovering another item switches to it. */
let openMenu = null;

items.system.innerHTML = icons.logo();

// --- menus -------------------------------------------------------------------------

function open(id, toggle) {
  const { left, top } = items[id].getBoundingClientRect();
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

listen("menu-opened", id => {
  openMenu = id;
  for (const [k, item] of Object.entries(items)) item.classList.toggle("open", k === id);
});
listen("menu-closed", () => {
  openMenu = null;
  for (const item of Object.values(items)) item.classList.remove("open");
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
  document.body.classList.toggle("vertical", vertical());
  items.stats.hidden = !settings.stats;
  renderFront();
  renderClock();
  renderStats();
  renderMedia();
  renderVolume();
  renderNetwork();
  renderBattery();
}

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
listen("settings", s => {
  settings = s;
  applyTheme(s);
  renderAll();
  tick();
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
  info = h;
  applyTheme(settings);
  renderAll();
  tick();
});
}
