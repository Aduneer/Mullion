"use strict";
// The menus. One window serves every menu: it lies along the bar, see-through, and the bar asks
// the backend to open a menu, which sends us its id and the item it hangs from. We lay it out,
// size the panel to it and place it against the bar under the item, then say so (`menu_ready`)
// and the backend shows the window. The panel grows out of the bar as it opens, slides and
// reshapes as other items' menus open, and folds back as it closes (menu.css).
{
const { invoke, listen, applyTheme, esc, bytes, rate, clock, duration, player, program, icons } = Lintel;
const root = document.getElementById("menu");
const panel = document.getElementById("panel");

/** The bar item the menu hangs from (in this window's CSS pixels), and the bar's edge. */
let anchor = { left: 0, top: 0, width: 0, height: 0 }, edge = "top";
/** The menu the backend has been told is laid out, so it's told once per menu. */
let readied = null;

let settings, info, sample, front;
/** The running apps, for their menus from the icons in the bar. */
let running = [];
/** The menu on screen (its bar item's id), and the view inside it (the same, or "settings", "confirm-…"). */
let current = null, view = null;
/** Handlers for the [data-act] elements of the current render, by index. */
let acts = [];
/** Last minute of CPU, memory and network, so the graphs have a past when the menu opens. */
const HISTORY = 60;
const hist = { cpu: [], mem: [], down: [], up: [] };
/** The graph under the pointer and the second it points at (0 = the oldest slot), for its readout. */
let hover = null;

// --- plumbing -------------------------------------------------------------------------

/** Close the menu; `restore` hands the focus back to the app that had it. */
const done = (restore = true) => invoke("close_menu", { restore });
const run = name => () => { done(false); invoke("run_action", { name }).catch(e => console.warn(e)); };
const act = (fn) => { acts.push(fn); return acts.length - 1; };

/** A menu item. `fn` returns "keep" to leave the menu open. */
function item(label, fn, { hint = "", tick = null, disabled = false, sub = "" } = {}) {
  const t = tick === null ? "" : `<span class="tick">${tick ? icons.check() : ""}</span>`;
  return `<button class="mi" role="menuitem" data-act="${act(fn)}"${disabled ? ' aria-disabled="true"' : ""}>${t}<span class="label">${esc(label)}${sub ? ` <span class="muted">${esc(sub)}</span>` : ""}</span>${hint ? `<span class="muted">${esc(hint)}</span>` : ""}</button>`;
}
const sep = `<div class="sep"></div>`;

/** Size the panel to the menu and place it against the bar, under its item but inside the
 *  bar's length. The first time for a menu, it then opens; after that it slides and reshapes. */
function measure() {
  if (!current) return;
  const GAP = 6;
  const w = Math.ceil(root.offsetWidth), h = Math.ceil(root.offsetHeight);
  const along = (from, len, size) => Math.max(GAP, Math.min(from, len - GAP - size));
  const [x, y] = {
    top: [along(anchor.left, innerWidth, w), 0],
    bottom: [along(anchor.left, innerWidth, w), innerHeight - h],
    left: [0, along(anchor.top, innerHeight, h)],
    right: [innerWidth - w, along(anchor.top, innerHeight, h)],
  }[edge];
  const opening = !panel.classList.contains("open");
  panel.classList.toggle("moving", !opening);
  Object.assign(panel.style, { left: `${x}px`, top: `${y}px`, width: `${w}px`, height: `${h}px` });
  if (readied === current) return;
  readied = current;
  const id = current;
  // Once the window is on screen, the menu grows out of the bar (from closed, at its place).
  invoke("menu_ready", { id }).then(() => requestAnimationFrame(() => {
    if (opening && current === id) panel.classList.add("open");
  }));
}
new ResizeObserver(measure).observe(root);

async function render() {
  const id = current;
  const build = VIEWS[view] ?? VIEWS[view.split(":")[0]];
  if (!build) return;
  const fresh = [];
  acts = fresh;
  const html = await build(view);
  if (current !== id || acts !== fresh) return; // another menu (or render) started meanwhile
  root.classList.toggle("wide", WIDE.has(view));
  root.innerHTML = html;
  kb = -1;
  measure();
}

root.addEventListener("click", e => {
  const el = e.target.closest("[data-act]");
  if (!el || el.getAttribute("aria-disabled") === "true") return;
  const r = acts[Number(el.dataset.act)]?.(el, e);
  if (r === undefined) done();
});

// Arrow keys move through the items, Enter picks one, Escape closes, as in any menu.
let kb = -1;
function moveKb(step) {
  const els = [...root.querySelectorAll(".mi[data-act]:not([aria-disabled]), .btn[data-act]")];
  if (!els.length) return;
  els[kb]?.classList.remove("kb");
  kb = kb < 0 ? (step > 0 ? 0 : els.length - 1) : (kb + step + els.length) % els.length;
  els[kb].classList.add("kb");
  els[kb].scrollIntoView({ block: "nearest" });
}
document.addEventListener("keydown", e => {
  if (e.key === "Escape") {
    if (view !== current) { view = current; render(); } else done();
  } else if (e.key === "ArrowDown") { moveKb(1); e.preventDefault(); }
  else if (e.key === "ArrowUp") { moveKb(-1); e.preventDefault(); }
  else if (e.key === "Enter") root.querySelector(".kb")?.click();
});
root.addEventListener("pointermove", () => {
  root.querySelector(".kb")?.classList.remove("kb");
  kb = -1;
});

// --- the menus --------------------------------------------------------------------------

/** Views whose content follows the samples: rebuilt every tick while open. */
const LIVE = new Set(["stats", "network", "battery"]);
const WIDE = new Set(["stats", "volume", "network", "battery", "bluetooth", "control"]);

const VIEWS = {
  system() {
    return [
      item("About This PC", run("ms-settings:about"), { sub: info.host }),
      sep,
      item("System Settings…", run("ms-settings:")),
      item("Task Manager", run("task-manager"), { hint: "Ctrl+Shift+Esc" }),
      info.oriel ? item("Oriel", run("oriel")) : "",
      sep,
      item("Sleep", () => { done(false); invoke("power", { action: "sleep" }); }),
      item("Restart…", () => confirm("restart")),
      item("Shut Down…", () => confirm("shutdown")),
      sep,
      item("Lock Screen", () => { done(false); invoke("power", { action: "lock" }); }, { hint: "Win+L" }),
      item("Sign Out…", () => confirm("signout")),
      sep,
      item("Menu Bar Settings…", () => { view = "settings"; render(); return "keep"; }),
      item("Quit Lintel", () => invoke("quit")),
    ].join("");
  },

  "confirm"(v) {
    const what = v.split(":")[1];
    const [question, verb] = {
      restart: ["Restart this PC now?", "Restart"],
      shutdown: ["Shut down this PC now?", "Shut Down"],
      signout: ["Sign out now?", "Sign Out"],
    }[what];
    return `<div class="confirm">
      <strong>${question}</strong>
      <p>Open apps will be asked to close. Ones with unsaved work may stop to ask, or lose it.</p>
      <div class="buttons">
        <button class="btn" data-act="${act(() => { view = current; render(); return "keep"; })}">Cancel</button>
        <button class="btn primary" data-act="${act(() => { done(false); invoke("power", { action: what }).catch(e => console.warn(e)); })}">${verb}</button>
      </div>
    </div>`;
  },

  async app() {
    if (!front || front.desktop) {
      return `<div class="head">Desktop</div>` + [
        item("File Explorer", run("explorer"), { hint: "Win+E" }),
        sep,
        item("Display Settings…", run("ms-settings:display")),
        item("Personalize…", run("ms-settings:personalization")),
      ].join("");
    }
    const a = running.find(x => x.windows.some(w => w.hwnd === front.hwnd));
    return appMenu({ name: front.name, pid: front.pid, key: a?.key });
  },

  /** A running app's menu, from its icon in the bar ("dock:<its key>"). */
  async dock(v) {
    const key = v.slice("dock:".length);
    const a = running.find(x => x.key === key);
    if (!a) return `<div class="note muted">${esc("That app has closed")}</div>`;
    return appMenu({ name: a.name, pid: a.pid, key });
  },

  async media() {
    const m = sample?.media;
    if (!m || !m.title) return `<div class="note muted">Nothing playing</div>`;
    const art = await coverFor(m);
    const progress = m.duration ? `<div class="progress"><span class="num" id="pos">${clock(position(m))}</span><div class="meter"><div id="bar" style="width:${(position(m) / m.duration) * 100}%"></div></div><span class="num">${clock(m.duration)}</span></div>` : "";
    const ctl = (name, icon, enabled, cls = "") => `<button class="${cls}" data-act="${act(() => { invoke("media_control", { action: name }); return "keep"; })}"${enabled ? "" : " disabled"}>${icon}</button>`;
    return `<div class="player">
      <div class="cover"${art ? ` style="background-image:url('${art}')"` : ""}>${art ? "" : icons.note()}</div>
      <div><div class="title">${esc(m.title)}</div><div class="artist">${esc([m.artist, m.album].filter(Boolean).join(" — "))}</div></div>
      ${progress}
      <div class="controls">
        ${ctl("previous", icons.prev(), m.can_prev)}
        ${ctl("toggle", m.playing ? icons.pause() : icons.play(), m.can_toggle, "main")}
        ${ctl("next", icons.next(), m.can_next)}
      </div>
      <div class="muted">Playing in ${esc(player(m.app) || "an app")}</div>
    </div>`;
  },

  stats() {
    const s = sample?.stats;
    if (!s) return `<div class="note muted">Waiting for the first reading…</div>`;
    const lvl = (p) => (p >= 92 ? "crit" : p >= 80 ? "warn" : "");
    return `
      <div class="section">
        <h3>CPU</h3>
        <div class="row"><span class="big ${lvl(s.cpu)}">${Math.round(s.cpu)}%</span></div>
        ${graph("cpu")}
      </div>
      <div class="section">
        <h3>Memory</h3>
        <div class="row"><span class="grow num">${bytes(s.mem.used)} of ${bytes(s.mem.total, 0)}</span><span class="num ${lvl(s.mem.pct)}">${Math.round(s.mem.pct)}%</span></div>
        ${graph("mem")}
      </div>
      <div class="section">
        <h3>Network</h3>
        <div class="row num"><span class="grow">↓ ${rate(s.net.down)}</span><span>↑ ${rate(s.net.up)}</span></div>
        ${graph("net")}
      </div>
      ${sep}
      ${info.oriel ? item("Open Oriel", run("oriel")) : ""}
      ${item("Task Manager", run("task-manager"), { hint: "Ctrl+Shift+Esc" })}`;
  },

  /** As on a Mac: the radios, the screens' brightness, sound and what's playing, in one place. */
  async control() {
    const r = sample?.radios ?? {};
    const link = sample?.stats.link;
    const a = sample?.audio, m = sample?.media;
    const displays = await invoke("displays").catch(() => []);
    const tile = (kind, label, sub, on, icon, detail) => `
      <div class="tile${on ? " on" : ""}">
        <button class="round" title="Turn ${label} ${on ? "off" : "on"}" data-act="${act(radio(kind, !on))}">${icon}</button>
        <button class="text" data-act="${act(detail)}"><strong>${label}</strong><span>${esc(sub)}</span></button>
      </div>`;
    const wifiSub = !r.wifi ? "Off" : link?.kind === "wifi" ? (link.ssid || "Connected") : "Not connected";
    const tiles = [
      r.wifi != null ? tile("wifi", "Wi-Fi", wifiSub, r.wifi, icons.wifi(r.wifi ? (link?.signal ?? 100) : 0), run("ms-availablenetworks:")) : "",
      r.bluetooth != null ? tile("bluetooth", "Bluetooth", r.bluetooth ? "On" : "Off", r.bluetooth, icons.bluetooth(r.bluetooth), () => { view = "bluetooth"; render(); return "keep"; }) : "",
    ].join("");
    const shortcut = (label, icon, fn, on, title) =>
      `<button class="shortcut${on ? " on" : ""}"${title ? ` title="${esc(title)}"` : ""} data-act="${act(fn)}"><span class="round">${icon}</span><span>${label}</span></button>`;
    const lit = displays.filter(d => d.brightness != null);
    const brightness = lit.map(d => `
      ${lit.length > 1 ? `<div class="caption">${esc(d.name)}</div>` : ""}
      <div class="slider" data-display="${esc(d.id)}">
        <span class="glyph">${icons.sun()}</span>
        <input type="range" min="0" max="100" value="${d.brightness}" aria-label="${esc(d.name)} brightness" title="${esc(d.name)}">
        <span class="pct">${d.brightness}%</span>
      </div>`).join("");
    const playing = m?.title ? `
      <div class="now">
        <button class="text" data-act="${act(() => { view = "media"; render(); return "keep"; })}"><strong>${esc(m.title)}</strong><span>${esc(m.artist || player(m.app))}</span></button>
        <button class="round small" title="${m.playing ? "Pause" : "Play"}" data-act="${act(() => { invoke("media_control", { action: "toggle" }); return "keep"; })}">${m.playing ? icons.pause() : icons.play()}</button>
      </div>` : "";
    return `
      ${tiles ? `<div class="tiles">${tiles}</div>` : ""}
      <div class="shortcuts">
        ${shortcut("Cast", icons.cast(), run("cast"))}
        ${shortcut("Project", icons.project(), run("project"))}
        ${shortcut("Night light", icons.moon(), run("ms-settings:nightlight"))}
        ${shortcut("Energy saver", icons.leaf(), run("ms-settings:batterysaver"), sample?.energy_saver, "Energy saver settings")}
      </div>
      ${brightness ? `<div class="section"><h3>${lit.length > 1 ? "Displays" : "Display"}</h3>${brightness}</div>` : ""}
      ${a ? `<div class="section"><h3>Sound</h3>
        <div class="slider" data-pid=""><button class="mute${a.muted ? " on" : ""}" title="${a.muted ? "Unmute" : "Mute"}" data-act="${act(() => { invoke("set_volume", { pid: null, muted: !a.muted }); a.muted = !a.muted; render(); return "keep"; })}">${icons.speaker(a.volume, a.muted || a.volume === 0)}</button>
        <input type="range" min="0" max="100" value="${a.volume}" aria-label="Volume"><span class="pct">${a.volume}%</span></div></div>` : ""}
      ${playing}
      ${sep}
      ${item("Notifications", run("notifications"), { hint: "Win+N" })}
      ${item("Quick Settings", run("quick-settings"), { hint: "Win+A" })}`;
  },

  async bluetooth() {
    const on = sample?.radios?.bluetooth;
    if (on == null) return `<div class="note muted">This PC has no Bluetooth</div>`;
    const devices = on ? await invoke("bluetooth_devices").catch(() => []) : [];
    const list = !on ? `<div class="note muted">Bluetooth is off</div>`
      : devices.length ? `<div class="section"><h3>Devices</h3></div>` + devices.map(d => item(d.name, run("ms-settings:bluetooth"), { tick: d.connected, sub: d.connected ? "Connected" : "" })).join("")
      : `<div class="note muted">No paired devices</div>`;
    return `
      <div class="head row"><span class="grow">Bluetooth</span>
        <label class="switch" title="Turn Bluetooth ${on ? "off" : "on"}"><input type="checkbox" data-act="${act(radio("bluetooth", !on))}"${on ? " checked" : ""}><span></span></label>
      </div>
      ${list}
      ${sep}
      ${item("Bluetooth Settings…", run("ms-settings:bluetooth"))}`;
  },

  volume() {
    const a = sample?.audio;
    if (!a) return `<div class="note muted">No sound output</div>` + sep + item("Sound Settings…", run("ms-settings:sound"));
    const slider = (pid, name, vol, muted) => `
      <div class="slider" data-pid="${pid ?? ""}">
        ${name ? `<span class="name" title="${esc(name)}">${esc(name)}</span>` : ""}
        <button class="mute${muted ? " on" : ""}" title="${muted ? "Unmute" : "Mute"}" data-act="${act(() => { invoke("set_volume", { pid, muted: !muted }); return "keep"; })}">${icons.speaker(vol, muted || vol === 0)}</button>
        <input type="range" min="0" max="100" value="${vol}" aria-label="${esc(name || "Volume")}">
        <span class="pct">${vol}%</span>
      </div>`;
    const apps = a.apps.filter(x => x.pid !== 0).concat(a.apps.filter(x => x.pid === 0));
    mixerKey = mixerShape(a);
    return `
      <div class="head">Sound <span class="sub">${esc(a.device)}</span></div>
      <div class="section">${slider(null, "", a.volume, a.muted)}</div>
      ${apps.length ? `<div class="section"><h3>Apps</h3>${apps.map(x => slider(x.pid, x.pid === 0 ? "System sounds" : x.title || program(x.exe), x.volume, x.muted)).join("")}</div>` : ""}
      ${sep}
      ${item("Sound Settings…", run("ms-settings:sound"))}
      ${item("Volume Mixer…", run("ms-settings:apps-volume"))}`;
  },

  network() {
    const l = sample?.stats.link;
    if (!l) return `<div class="note muted">Checking…</div>`;
    const status = l.kind === "offline" ? "Not connected" : l.internet ? "Connected" : "Connected, no internet";
    const body = l.kind === "wifi"
      ? `<div class="head">Wi-Fi</div><div class="section"><div class="row">${icons.wifi(l.signal ?? 100)}<span class="grow"><strong>${esc(l.ssid || "Connected")}</strong></span>${l.signal == null ? "" : `<span class="muted num">${l.signal}%</span>`}</div><div class="muted">${status}</div></div>`
      : `<div class="head">${l.kind === "wired" ? "Network" : "Offline"}</div><div class="section"><div class="row">${l.kind === "wired" ? icons.wired() : icons.offline()}<span class="grow">${status}</span></div></div>`;
    return body + sep + item("Wi-Fi Networks…", run("ms-availablenetworks:")) + item("Network & Internet Settings…", run("ms-settings:network"));
  },

  battery() {
    const b = sample?.stats.battery;
    if (!b) return `<div class="note muted">No battery</div>`;
    const status = b.plugged ? (b.pct >= 100 ? "Fully charged" : "Charging") : b.secs_left ? `On battery · ${duration(b.secs_left)} left` : "On battery";
    return `
      <div class="head">Battery</div>
      <div class="section">
        <div class="row"><span class="big">${b.pct}%</span></div>
        <div class="meter" style="--hue:${b.pct <= 20 && !b.plugged ? "var(--crit)" : "var(--mem)"}"><div style="width:${b.pct}%"></div></div>
        <div class="muted">${status}</div>
      </div>
      ${sep}
      ${item("Power & Battery Settings…", run("ms-settings:powersleep"))}`;
  },

  clock() {
    const now = new Date();
    return `<div class="head">${esc(now.toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long", year: "numeric" }))}</div>`
      + calendar() + sep
      + item("Notification Centre", run("notifications"), { hint: "Win+N" })
      + item("Date & Time Settings…", run("ms-settings:dateandtime"));
  },

  async settings() {
    const s = settings;
    const [screens, canHide] = await Promise.all([invoke("screens").catch(() => []), invoke("taskbar_can_hide").catch(() => true)]);
    const save = patch => () => { invoke("save_settings", { settings: { ...settings, ...patch } }); return "keep"; };
    const seg = (key, options) => `<div class="seg">${options.map(([v, label]) => `<button aria-pressed="${s[key] === v}" data-act="${act(save({ [key]: v }))}">${label}</button>`).join("")}</div>`;
    const check = (key, label) => `<label class="toggle"><input type="checkbox" data-act="${act(save({ [key]: !s[key] }))}"${s[key] ? " checked" : ""}>${label}</label>`;
    // Screens by number, as Windows' Display settings does: the main one is 1.
    const screenSeg = screens.length > 1
      ? `<div class="field"><span>Screens</span>${seg("screens", [["main", "Main"], ["all", "All"], ...screens.slice(1).map((x, i) => [x.id, `Screen ${i + 2}`])])}</div>`
      : "";
    const PALETTES = [["oriel", "Default"], ["catppuccin", "Catppuccin"], ["nord", "Nord"], ["gruvbox", "Gruvbox"], ["rose-pine", "Rosé Pine"], ["solarized", "Solarized"]];
    const mode = document.documentElement.dataset.mode;
    return `
      <div class="head">Menu Bar Settings</div>
      <div class="form">
        <div class="field"><span>Appearance</span>${seg("theme", [["auto", "Auto"], ["light", "Light"], ["dark", "Dark"]])}</div>
        <div class="field"><span>Material</span>${seg("glass", [[false, "Solid"], [true, "Glass"]])}</div>
        <div class="field"><span>Palette</span><div class="swatches">${PALETTES.map(([v, label]) => `<button class="swatch" title="${label}" data-palette="${v}" data-mode="${mode}" aria-pressed="${s.palette === v}" data-act="${act(save({ palette: v }))}"><span></span></button>`).join("")}</div></div>
        ${screenSeg}
        <div class="field"><span>Position</span>${seg("edge", [["top", "Top"], ["bottom", "Bottom"], ["left", "Left"], ["right", "Right"]])}</div>
        <div class="field"><span>Size</span>${seg("height", [[26, "Compact"], [30, "Standard"], [36, "Large"]])}</div>
        <div class="field"><span>Clock</span>${seg("clock", [["auto", "Auto"], ["12", "12-hour"], ["24", "24-hour"]])}</div>
        <div class="checks">${check("date", "Show the date")}${check("seconds", "Show seconds")}</div>
        <h3 class="muted" style="font-size:11px;letter-spacing:.04em;text-transform:uppercase;margin-top:4px">Show in the bar</h3>
        <div class="checks">
          ${check("apps", "Open apps")}${screens.length > 1 && s.screens === "all" ? check("apps_per_screen", "Only this screen's apps") : ""}${check("media", "Now playing")}${check("stats", "CPU & memory")}
          ${check("volume", "Volume")}${check("network", "Network")}
          ${check("bluetooth", "Bluetooth")}${check("control", "Control Centre")}
          ${info.has_battery ? check("battery", "Battery") : ""}
        </div>
        <label class="toggle" title="Windows' taskbar auto-hides while Lintel runs, and comes back when Lintel quits"><input type="checkbox" data-act="${act(save({ hide_taskbar: !s.hide_taskbar }))}"${s.hide_taskbar ? " checked" : ""}${canHide || s.hide_taskbar ? "" : " disabled"}>Hide the Windows taskbar</label>
        ${canHide ? "" : `<div class="hint muted">Windows only hides a taskbar at the bottom of the screen</div>`}
        <label class="toggle"><input type="checkbox" data-act="${act(el => { invoke("set_autostart", { enabled: el.checked }).then(on => { info.autostart = on; el.checked = on; }); return "keep"; })}"${info.autostart ? " checked" : ""}>Start Lintel when I sign in</label>
      </div>
      ${sep}
      ${item("Back", () => { view = current; render(); return "keep"; })}
      <div class="foot">Lintel ${esc(info.version)} · settings in ${esc(info.settings_path)}</div>`;
  },
};

/** An app's windows to switch to, a new window, and minimize, close and quit. */
async function appMenu(f) {
  const wins = await invoke("app_windows", { pid: f.pid }).catch(() => []);
  const frontWin = wins.find(w => w.front) ?? wins[0];
  const winItem = w => item(w.title, () => { done(false); invoke("window_action", { action: "focus", hwnd: w.hwnd, pid: w.pid }); }, { tick: w.front, sub: w.minimized ? "minimized" : "" });
  return `<div class="head">${esc(f.name)}</div>` + [
    ...wins.slice(0, 15).map(winItem),
    wins.length > 15 ? `<div class="note muted">and ${wins.length - 15} more</div>` : "",
    wins.length ? "" : `<div class="note muted">No windows open</div>`,
    sep,
    f.key ? item("New Window", () => { done(false); invoke("launch_app", { key: f.key }).catch(e => console.warn(e)); }, { hint: "Middle click" }) : "",
    item("Minimize", () => { done(false); invoke("window_action", { action: "minimize", hwnd: frontWin.hwnd, pid: frontWin.pid }); }, { disabled: !frontWin, hint: "Win+↓" }),
    item("Close Window", () => { done(false); invoke("window_action", { action: "close", hwnd: frontWin.hwnd, pid: frontWin.pid }); }, { disabled: !frontWin, hint: "Alt+F4" }),
    sep,
    item(`Quit ${f.name}`, () => { done(false); invoke("quit_app", { pid: f.pid }).catch(e => console.warn(e)); }, { disabled: !wins.length }),
  ].join("");
}

/** Turn a radio on or off, showing it at once; the next samples confirm it. */
function radio(kind, on) {
  return () => {
    if (sample?.radios) sample.radios[kind] = on;
    invoke("set_radio", { kind, on }).catch(e => { console.warn(e); }).finally(() => render());
    render();
    return "keep";
  };
}

function confirm(what) {
  view = `confirm:${what}`;
  render();
  return "keep";
}

// --- pieces --------------------------------------------------------------------------

/** The graphs in the stats menu: which history each draws, its scale and color, and its readout. */
const GRAPHS = {
  cpu: { values: () => hist.cpu, max: 100, hue: "var(--cpu)", read: i => `${hist.cpu[i].toFixed(1)}%` },
  mem: { values: () => hist.mem, max: 100, hue: "var(--mem)", read: i => `${hist.mem[i].toFixed(1)}%` },
  net: { values: () => hist.down, max: null, hue: "var(--s2)", read: i => `↓ ${rate(hist.down[i])}  ↑ ${rate(hist.up[i])}` },
};

/** A graph of the last minute. Under the pointer it marks a second and says exactly what it was then. */
function graph(key) {
  const { values: get, max, hue, read } = GRAPHS[key];
  const values = get();
  if (values.length < 2) return `<div class="graph" data-graph="${key}"><svg class="spark"></svg></div>`;
  const top = max ?? Math.max(1, ...values) * 1.15;
  const n = HISTORY - 1;
  const x = i => ((HISTORY - values.length + i) / n) * 100;
  const y = v => 40 - (Math.min(v, top) / top) * 38;
  const pts = values.map((v, i) => `${x(i)},${y(v)}`);
  let mark = "";
  if (hover?.graph === key) {
    const i = Math.max(0, Math.min(values.length - 1, hover.slot - (HISTORY - values.length)));
    const ago = values.length - 1 - i;
    const right = x(i) > 60;
    mark = `<div class="mark" style="left:${x(i)}%"><i style="top:${y(values[i])}px"></i></div>
      <div class="readout num" style="${right ? `right:${100 - x(i)}%` : `left:${x(i)}%`}">${esc(read(i))} <span class="muted">${ago ? `${ago}s ago` : "now"}</span></div>`;
  }
  return `<div class="graph" data-graph="${key}" style="--hue:${hue}">
    <svg class="spark" viewBox="0 0 100 40" preserveAspectRatio="none">
    <path class="area" d="M${x(0)},40 L${pts.join(" L")} L100,40 Z"/><path class="line" d="M${pts.join(" L")}"/></svg>${mark}</div>`;
}

root.addEventListener("pointermove", e => {
  const el = e.target.closest(".graph");
  const key = el?.dataset.graph;
  if (!key) return;
  const r = el.getBoundingClientRect();
  const slot = Math.round(((e.clientX - r.left) / r.width) * (HISTORY - 1));
  if (hover?.graph === key && hover.slot === slot) return;
  hover = { graph: key, slot };
  el.outerHTML = graph(key);
});
root.addEventListener("pointerout", e => {
  const el = e.target.closest(".graph");
  if (!el || el.contains(e.relatedTarget) || hover?.graph !== el.dataset.graph) return;
  hover = null;
  el.outerHTML = graph(el.dataset.graph);
});

/** Where the track is now: players report a position and when, and it moves on while playing. */
function position(m) {
  const p = (m.position ?? 0) + (m.playing ? Date.now() / 1000 - m.at : 0);
  return Math.min(m.duration ?? p, Math.max(0, p));
}

let cover = { id: 0, url: null };
async function coverFor(m) {
  if (!m.art) return null;
  if (cover.id !== m.art) cover = { id: m.art, url: await invoke("media_art").catch(() => null) };
  return cover.url;
}

// The month shown; the arrows move it, "Today" (and reopening the menu) brings it back.
let month = null;
function calendar() {
  const today = new Date();
  month ??= new Date(today.getFullYear(), today.getMonth(), 1);
  // Windows numbers days from Monday (0); Date from Sunday (0).
  const first = (info.first_day + 1) % 7;
  const start = new Date(month);
  start.setDate(1 - ((month.getDay() - first + 7) % 7));
  const dows = [...Array(7)].map((_, i) => new Date(2024, 0, 7 + first + i).toLocaleDateString(undefined, { weekday: "narrow" }));
  const days = [...Array(42)].map((_, i) => {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    const cls = ["day", d.getMonth() !== month.getMonth() ? "other" : "", d.toDateString() === today.toDateString() ? "today" : ""].join(" ");
    return `<div class="${cls}">${d.getDate()}</div>`;
  });
  const go = delta => () => { month = new Date(month.getFullYear(), month.getMonth() + delta, 1); render(); return "keep"; };
  const isThisMonth = month.getFullYear() === today.getFullYear() && month.getMonth() === today.getMonth();
  return `<div class="cal">
    <header>
      <strong>${esc(month.toLocaleDateString(undefined, { month: "long", year: "numeric" }))}</strong>
      <span class="row">
        ${isThisMonth ? "" : `<button class="today-btn" data-act="${act(() => { month = null; render(); return "keep"; })}">Today</button>`}
        <button title="Previous month" data-act="${act(go(-1))}">${icons.chevron("left")}</button>
        <button title="Next month" data-act="${act(go(1))}">${icons.chevron("right")}</button>
      </span>
    </header>
    <div class="grid">${dows.map(d => `<div class="dow">${esc(d)}</div>`).join("")}${days.join("")}</div>
  </div>`;
}

// The mixer is rebuilt only when its rows change, so a slider being dragged isn't replaced under the pointer.
let mixerKey = "";
const mixerShape = a => `${a.device}|${a.apps.map(x => x.pid).join(",")}`;
let dragging = false;
root.addEventListener("pointerdown", e => { dragging = e.target.matches("input[type=range]"); });
document.addEventListener("pointerup", () => { dragging = false; });
root.addEventListener("input", e => {
  const el = e.target;
  if (!el.matches("input[type=range]")) return;
  const row = el.closest(".slider");
  row.querySelector(".pct").textContent = `${el.value}%`;
  if (row.dataset.display) return setBrightness(row.dataset.display, Number(el.value));
  const pid = row.dataset.pid === "" ? null : Number(row.dataset.pid);
  invoke("set_volume", { pid, volume: Number(el.value) });
});

// A monitor takes a moment to change its brightness, so while a slider moves only the latest
// value is sent, one at a time.
let brightnessBusy = false, brightnessNext = null;
async function setBrightness(id, value) {
  brightnessNext = { id, value };
  if (brightnessBusy) return;
  brightnessBusy = true;
  while (brightnessNext) {
    const v = brightnessNext;
    brightnessNext = null;
    await invoke("set_brightness", v).catch(e => console.warn(e));
  }
  brightnessBusy = false;
}

function updateVolume() {
  const a = sample?.audio;
  if (!a || mixerShape(a) !== mixerKey) return render();
  if (dragging) return;
  const rows = root.querySelectorAll(".slider");
  const values = [{ volume: a.volume, muted: a.muted }, ...a.apps.filter(x => x.pid !== 0), ...a.apps.filter(x => x.pid === 0)];
  rows.forEach((row, i) => {
    const v = values[i];
    if (!v) return;
    row.querySelector("input").value = v.volume;
    row.querySelector(".pct").textContent = `${v.volume}%`;
    const mute = row.querySelector(".mute");
    if (mute.classList.contains("on") !== v.muted) render();
  });
}

let lastMedia = "";
function updateMedia() {
  const m = sample?.media;
  const key = m ? `${m.title}|${m.artist}|${m.playing}|${m.art}|${m.can_prev}|${m.can_next}` : "";
  if (key !== lastMedia) { lastMedia = key; return render(); }
}
setInterval(() => {
  const m = sample?.media;
  if (view !== "media" || !m?.duration) return;
  const pos = document.getElementById("pos"), bar = document.getElementById("bar");
  if (pos) pos.textContent = clock(position(m));
  if (bar) bar.style.width = `${(position(m) / m.duration) * 100}%`;
}, 500);

// --- data ----------------------------------------------------------------------------

listen("menu", m => {
  anchor = m.item;
  edge = m.edge;
  document.body.dataset.edge = edge;
  // Another item's menu while one is open: the old one fades out as the new one fades in.
  if (current && panel.classList.contains("open")) {
    const ghost = root.cloneNode(true);
    ghost.removeAttribute("id");
    ghost.classList.add("ghost");
    panel.append(ghost);
    setTimeout(() => ghost.remove(), 150);
    root.classList.remove("entering");
    void root.offsetWidth;
    root.classList.add("entering");
  }
  current = view = m.id;
  month = null;
  lastMedia = "";
  if (settings?.glass) refreshWall();
  render();
});
listen("menu-closed", () => {
  current = view = hover = null;
  readied = null;
  // It folds back into the bar; the backend hides the window once it has.
  panel.classList.remove("open", "moving");
  setTimeout(() => { if (!current) root.innerHTML = ""; }, 200);
});
// A click beside the menu (the window is larger than it) closes it, as one anywhere else does.
document.addEventListener("pointerdown", e => {
  if (current && !e.target.closest("#panel")) invoke("close_menu", { restore: true });
});

// With glass, the panel shows the wallpaper behind this window, blurred as the bar's is
// (common.js); menu.css fixes it to the window, so it carries straight on from the bar's.
const wallCanvas = document.createElement("canvas");
let wallKey = null, wallUrl = null;
async function refreshWall() {
  const key = await Lintel.backdrop(wallCanvas, wallKey);
  if (!key || key === wallKey) return;
  wallKey = key;
  const blob = await new Promise(ok => wallCanvas.toBlob(ok));
  if (!blob) return;
  if (wallUrl) URL.revokeObjectURL(wallUrl);
  wallUrl = URL.createObjectURL(blob);
  document.documentElement.style.setProperty("--wall", `url("${wallUrl}")`);
}
listen("sample", s => {
  const toggles = s => JSON.stringify([s?.radios, s?.energy_saver]);
  const was = toggles(sample);
  sample = s;
  for (const [k, v] of [["cpu", s.stats.cpu], ["mem", s.stats.mem.pct], ["down", s.stats.net.down], ["up", s.stats.net.up]]) {
    hist[k].push(v);
    if (hist[k].length > HISTORY) hist[k].shift();
  }
  if (!current) return;
  if (LIVE.has(view)) render();
  else if (view === "volume") updateVolume();
  else if (view === "media") updateMedia();
  // A radio turned on or off elsewhere (or by us, confirmed).
  else if ((view === "control" || view === "bluetooth") && toggles(s) !== was) render();
});
listen("front", f => { front = f; });
listen("apps", a => { running = a; });
listen("settings", s => {
  settings = s;
  applyTheme(s);
  if (view === "settings") render();
});

invoke("hello").then(h => {
  ({ settings, sample, front } = h);
  running = h.apps ?? [];
  info = h;
  applyTheme(settings);
});
}
