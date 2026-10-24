// The preview's stand-in for the Rust backend: made-up but plausible data, and the bar/menu window
// dance (open_menu → "menu" event → menu_ready → place and show) done with two iframes.
//
// URL options, all optional:
//   theme=auto|dark|light  palette=nord  height=30  edge=top|bottom|left|right  open=clock (a menu open at start)
//   media=off  battery=off  wifi=off (wired)  offline=1  desktop=1 (the desktop in front)  hot=1 (high CPU and memory)
"use strict";
(() => {
  const q = new URLSearchParams(location.search);
  const $ = id => document.getElementById(id);

  // The same fixed clock mock.js gives the pages (?time=…), so the track's position agrees.
  if (q.get("time")) {
    const RealDate = Date;
    const offset = new RealDate(q.get("time")).getTime() - RealDate.now();
    if (Number.isFinite(offset)) {
      globalThis.Date = class extends RealDate {
        constructor(...a) { super(...(a.length ? a : [RealDate.now() + offset])); }
        static now() { return RealDate.now() + offset; }
      };
    }
  }
  const barFrame = $("bar"), menuFrame = $("menu");
  const log = msg => { $("log").textContent = msg; clearTimeout(log.t); log.t = setTimeout(() => ($("log").textContent = ""), 4000); };

  const GB = 1024 ** 3;
  let settings = {
    theme: q.get("theme") || "auto", palette: q.get("palette") || "oriel", glass: q.get("glass") === "on", height: Number(q.get("height")) || 30, edge: q.get("edge") || "top",
    clock: "auto", seconds: false, date: true, apps: q.get("apps") !== "off", media: true, stats: true, volume: true, network: true, battery: true,
  };
  let autostart = false;
  const hasBattery = q.get("battery") !== "off";
  const front = q.get("desktop")
    ? { hwnd: 1, pid: 4, exe: "explorer", name: "Desktop", desktop: true }
    : { hwnd: 4242, pid: 812, exe: "chrome", name: "Google Chrome", desktop: false };
  const windows = [
    { hwnd: 4242, pid: 812, title: "Lintel — a menu bar for Windows - GitHub", minimized: false, front: true },
    { hwnd: 4243, pid: 812, title: "Inbox (3) - Gmail", minimized: false, front: false },
    { hwnd: 4244, pid: 812, title: "YouTube Music", minimized: true, front: false },
  ];
  let media = q.get("media") === "off" ? null : {
    app: "Spotify.exe", title: "Midnight City", artist: "M83", album: "Hurry Up, We're Dreaming", playing: true,
    position: 74, duration: 243, at: Date.now() / 1000, can_prev: true, can_next: true, can_toggle: true, art: 1,
  };
  const audio = {
    device: "Speakers (Realtek(R) Audio)", volume: 42, muted: false,
    apps: [
      { pid: 812, exe: "chrome", title: "", volume: 100, muted: false, active: true },
      { pid: 990, exe: "Spotify", title: "", volume: 80, muted: false, active: true },
      { pid: 0, exe: "", title: "", volume: 100, muted: false, active: false },
    ],
  };
  const hot = q.get("hot");
  let t = 0;
  function sample() {
    t++;
    const cpu = hot ? 94 + Math.sin(t) * 3 : 18 + Math.sin(t / 4) * 9 + Math.sin(t * 1.7) * 4;
    const memPct = hot ? 91 : 58.4;
    return {
      t: Date.now() / 1000,
      stats: {
        cpu: Math.round(cpu * 10) / 10,
        mem: { used: memPct / 100 * 16 * GB, total: 16 * GB, pct: memPct },
        net: { down: 1.2e6 + Math.abs(Math.sin(t / 3)) * 3.5e6, up: 9e4 + Math.abs(Math.cos(t / 2)) * 1.2e5 },
        battery: hasBattery ? { pct: 76, plugged: false, secs_left: 3 * 3600 + 25 * 60 } : null,
        link: q.get("offline") ? { kind: "offline", internet: false, ssid: "", signal: 0 }
          : q.get("wifi") === "off" ? { kind: "wired", internet: true, ssid: "", signal: 0 }
          : { kind: "wifi", internet: true, ssid: "Home 5G", signal: 82 },
      },
      media: media && { ...media },
      audio: structuredClone(audio),
    };
  }
  let latest = sample();

  // --- events ---------------------------------------------------------------------------
  const listeners = { bar: {}, menu: {} };
  function listen(label, event, fn) {
    (listeners[label][event] ??= []).push(fn);
    return () => {};
  }
  function emitTo(label, event, payload) {
    for (const fn of listeners[label][event] ?? []) fn(structuredClone(payload));
  }
  const emit = (event, payload) => { emitTo("bar", event, payload); emitTo("menu", event, payload); };

  // --- menu window -------------------------------------------------------------------------
  let open = null; // [id, left]
  function hideMenu() {
    if (!open) return;
    open = null;
    menuFrame.style.display = "none";
    emit("menu-closed", null);
  }
  document.addEventListener("pointerdown", e => { if (e.target === document.body || e.target.closest(".window")) hideMenu(); });

  // A cover for the fake track: a gradient, as an SVG data: URL.
  const art = "data:image/svg+xml;base64," + btoa(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff7a59"/><stop offset=".5" stop-color="#b03ab8"/><stop offset="1" stop-color="#2b2d7a"/></linearGradient></defs><rect width="10" height="10" fill="url(#g)"/><circle cx="7" cy="3" r="1.4" fill="#ffd9a8" opacity=".9"/></svg>`);

  // A made-up wallpaper with some detail, so the glass has something to blur.
  const wallpaperArt = "data:image/svg+xml;base64," + btoa(`<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff9a62"/><stop offset=".45" stop-color="#7b4fc9"/><stop offset="1" stop-color="#1c2a6b"/></linearGradient></defs><rect width="1600" height="900" fill="url(#g)"/><circle cx="1180" cy="120" r="170" fill="#ffd27a" opacity=".85"/><circle cx="300" cy="60" r="120" fill="#5fd0ff" opacity=".7"/><path d="M0 700 Q400 520 800 690 T1600 640 V900 H0Z" fill="#14183a" opacity=".8"/></svg>`);

  const commands = {
    hello: () => ({
      settings, sample: latest, front, has_battery: hasBattery, oriel: true, autostart, clock24: true, first_day: 0,
      version: "0.1.0", host: "DESKTOP-7Q2K4M", settings_path: "C:\\Users\\you\\AppData\\Roaming\\Lintel\\settings.json",
    }),
    open_menu({ id, item, toggle }) {
      if (open?.[0] === id) { if (toggle) hideMenu(); return; }
      open = [id, item];
      emitTo("menu", "menu", id);
    },
    menu_ready({ id, width, height }) {
      if (open?.[0] !== id) return;
      // The same rules as place_menu in main.rs.
      const b = barFrame.getBoundingClientRect(), it = open[1];
      const along = (start, len, from, size) => Math.max(start + 6, Math.min(start + from, start + len - 6 - size));
      const [x, y] = {
        top: [along(b.left, b.width, it.left, width), b.bottom + 4],
        bottom: [along(b.left, b.width, it.left, width), b.top - 4 - height],
        left: [b.right + 4, along(b.top, b.height, it.top, height)],
        right: [b.left - 4 - width, along(b.top, b.height, it.top, height)],
      }[settings.edge];
      Object.assign(menuFrame.style, { left: `${x}px`, top: `${y}px`, width: `${width}px`, height: `${height}px`, display: "block" });
      menuFrame.contentWindow.focus();
      emitTo("bar", "menu-opened", id);
    },
    close_menu: () => hideMenu(),
    media_control({ action }) {
      if (action === "toggle") media.playing = !media.playing;
      if (action === "next") Object.assign(media, { title: "Wait", position: 0, art: 2 });
      log(`media: ${action}`);
    },
    media_art: () => art,
    // The harness page's own background stands in for the wallpaper behind the bar.
    wallpaper() {
      const r = barFrame.getBoundingClientRect(), d = devicePixelRatio;
      const screen = [0, 0, innerWidth * d, innerHeight * d].map(Math.round);
      return { key: "preview", color: "#1c2238", position: "fill", monitor: screen, desktop: screen, bar: [r.left, r.top, r.width, r.height].map(v => Math.round(v * d)) };
    },
    wallpaper_image: () => wallpaperArt,
    set_volume({ pid, volume, muted }) {
      const target = pid == null ? audio : audio.apps.find(a => a.pid === pid);
      if (volume != null) target.volume = Math.round(volume);
      if (muted != null) target.muted = muted;
    },
    app_windows: () => windows,
    window_action: a => log(`window: ${a.action} ${a.hwnd}`),
    quit_app: a => log(`quit app ${a.pid}`),
    power: a => log(`power: ${a.action}`),
    run_action: a => log(`open: ${a.name}`),
    save_settings({ settings: s }) {
      settings = s;
      applySize();
      emit("settings", settings);
      return settings;
    },
    set_autostart: ({ enabled }) => (autostart = enabled),
    quit: () => log("quit"),
  };

  window.LintelHost = {
    listen,
    invoke(label, cmd, args) {
      const fn = commands[cmd];
      if (!fn) return Promise.reject(`unknown command ${cmd}`);
      try { return Promise.resolve(fn(args)); } catch (e) { return Promise.reject(String(e)); }
    },
  };

  function applySize() {
    const side = settings.edge === "left" || settings.edge === "right";
    const depth = side ? settings.height + 16 : settings.height;
    Object.assign(barFrame.style, side
      ? { width: `${depth}px`, height: "100%", top: "0", bottom: "auto", left: settings.edge === "left" ? "0" : "auto", right: settings.edge === "right" ? "0" : "auto" }
      : { width: "100%", height: `${depth}px`, left: "0", right: "auto", top: settings.edge === "top" ? "0" : "auto", bottom: settings.edge === "bottom" ? "0" : "auto" });
    const dark = settings.theme === "dark" || (settings.theme === "auto" && matchMedia("(prefers-color-scheme: dark)").matches);
    document.body.classList.toggle("light", !dark);
    document.body.classList.toggle("glass", settings.glass);
    document.body.style.setProperty("--wallpaper", `url("${wallpaperArt}")`);
  }
  applySize();
  barFrame.src = "/ui/index.html";
  menuFrame.src = "/ui/menu.html";
  // A minute of history at once, so the graphs in the stats menu have something to show.
  menuFrame.addEventListener("load", () => setTimeout(() => {
    for (let i = 0; i < 59; i++) emitTo("menu", "sample", sample());
  }, 300), { once: true });

  setInterval(() => {
    latest = sample();
    emit("sample", latest);
  }, 1000);

  if (q.get("open")) {
    setTimeout(() => {
      const item = barFrame.contentDocument.querySelector(`[data-menu="${q.get("open")}"]`);
      const { left, top } = item.getBoundingClientRect();
      commands.open_menu({ id: q.get("open"), item: { left, top }, toggle: false });
    }, 1500);
  }
})();
