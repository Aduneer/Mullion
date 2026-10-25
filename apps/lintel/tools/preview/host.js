// The preview's stand-in for the Rust backend: made-up but plausible data, and the bar/menu window
// dance (open_menu → "menu" event → menu_ready → place and show) done with two iframes.
//
// URL options, all optional:
//   theme=auto|dark|light  palette=nord  height=30  edge=top|bottom|left|right  open=clock (a menu open at start)
//   media=off  battery=off  wifi=off (wired)  offline=1  desktop=1 (the desktop in front)  hot=1 (high CPU and memory)
//   screens=2 (a second screen)  glass=on  apps=off (no open apps)
//   demo=1 (a stand-in pointer opens a menu and slides along the bar, for the README's animation)
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
    clock: "auto", seconds: false, date: true, bluetooth: true, control: true, apps: q.get("apps") !== "off", apps_per_screen: true, screens: "main", media: true, stats: true, volume: true, network: true, battery: true, hide_taskbar: false,
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
  // The open apps in the middle of the bar, with plain made-up icons (no real apps' logos).
  const appIcon = (from, to, glyph) => "data:image/svg+xml;base64," + btoa(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs><rect x="4" y="4" width="56" height="56" rx="14" fill="url(#g)"/><g fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">${glyph}</g></svg>`);
  const win = (hwnd, pid, extra = {}) => ({ hwnd, pid, minimized: false, front: false, monitor: 1, ...extra });
  const running = q.get("apps") === "off" ? [] : [
    { key: "chrome", name: "Google Chrome", exe: "chrome", pid: 812, icon: appIcon("#5aa0f2", "#2f6fd1", `<circle cx="32" cy="32" r="15"/><path d="M17 32h30M32 17c-6 8-6 22 0 30M32 17c6 8 6 22 0 30"/>`),
      windows: [win(4242, 812, { front: true }), win(4243, 812), win(4244, 812, { minimized: true })] },
    { key: "explorer", name: "File Explorer", exe: "explorer", pid: 4, icon: appIcon("#ffd36b", "#f0a92e", `<path d="M16 22h12l4 4h16v16H16z"/>`), windows: [win(5001, 4)] },
    { key: "terminal", name: "Terminal", exe: "WindowsTerminal", pid: 1210, icon: appIcon("#4a4f5c", "#22252c", `<path d="M19 24l8 8-8 8M31 41h14"/>`), windows: [win(5002, 1210)] },
    { key: "spotify", name: "Spotify", exe: "Spotify", pid: 990, icon: appIcon("#4fd18a", "#1f9d57", `<path d="M26 40V22l16-4v18"/><circle cx="23" cy="40" r="3"/><circle cx="39" cy="36" r="3"/>`), windows: [win(5003, 990)] },
    { key: "code", name: "Visual Studio Code", exe: "Code", pid: 1404, icon: appIcon("#7a8cff", "#4a5ae0", `<path d="M26 22l-10 10 10 10M38 22l10 10-10 10"/>`), windows: [win(5004, 1404), win(5005, 1404)] },
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
  const radios = { wifi: q.get("offline") ? false : true, bluetooth: true };
  let displayLevel = 70;
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
      radios: { ...radios },
      energy_saver: false,
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
  // As in main.rs: the menu window lies along the bar, see-through, room enough for any menu, and
  // the menu page draws the menu in it. Closing, it folds back first, then the window goes.
  const ROOM = [560, 900];
  let open = null; // [id, item]
  function hideMenu() {
    if (!open) return;
    open = null;
    emit("menu-closed", null);
    setTimeout(() => { if (!open) menuFrame.style.visibility = "hidden"; }, 220);
  }
  function placeStage() {
    const b = barFrame.getBoundingClientRect();
    const [x, y, w, h] = {
      top: [b.left, b.bottom, b.width, ROOM[1]],
      bottom: [b.left, b.top - ROOM[1], b.width, ROOM[1]],
      left: [b.right, b.top, ROOM[0], b.height],
      right: [b.left - ROOM[0], b.top, ROOM[0], b.height],
    }[settings.edge];
    Object.assign(menuFrame.style, { left: `${x}px`, top: `${y}px`, width: `${w}px`, height: `${h}px` });
  }
  document.addEventListener("pointerdown", e => { if (e.target === document.body || e.target.closest(".window")) hideMenu(); });

  // A cover for the fake track: a gradient, as an SVG data: URL.
  const art = "data:image/svg+xml;base64," + btoa(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff7a59"/><stop offset=".5" stop-color="#b03ab8"/><stop offset="1" stop-color="#2b2d7a"/></linearGradient></defs><rect width="10" height="10" fill="url(#g)"/><circle cx="7" cy="3" r="1.4" fill="#ffd9a8" opacity=".9"/></svg>`);

  // A made-up wallpaper with some detail, so the glass has something to blur.
  const wallpaperArt = "data:image/svg+xml;base64," + btoa(`<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff9a62"/><stop offset=".45" stop-color="#7b4fc9"/><stop offset="1" stop-color="#1c2a6b"/></linearGradient></defs><rect width="1600" height="900" fill="url(#g)"/><circle cx="1180" cy="120" r="170" fill="#ffd27a" opacity=".85"/><circle cx="300" cy="60" r="120" fill="#5fd0ff" opacity=".7"/><path d="M0 700 Q400 520 800 690 T1600 640 V900 H0Z" fill="#14183a" opacity=".8"/></svg>`);

  const commands = {
    hello: () => ({
      settings, sample: latest, front, apps: running, has_battery: hasBattery, oriel: true, autostart, clock24: true, first_day: 0,
      version: "0.1.0", host: "DESKTOP-7Q2K4M", settings_path: "C:\\Users\\you\\AppData\\Roaming\\Lintel\\settings.json",
    }),
    open_menu({ id, item, toggle }) {
      if (open?.[0] === id) { if (toggle) hideMenu(); return; }
      if (!open) placeStage();
      open = [id, item];
      emitTo("menu", "menu", { id, item, edge: settings.edge });
    },
    menu_ready({ id }) {
      if (open?.[0] !== id) return;
      menuFrame.style.visibility = "visible";
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
    wallpaper(_, label) {
      // The menu frame may still be hidden here, as the real window may: use where it's been put.
      const m = menuFrame.style, px = v => parseFloat(v) || 0;
      const r = label === "menu" ? { left: px(m.left), top: px(m.top), width: px(m.width), height: px(m.height) } : barFrame.getBoundingClientRect();
      const d = devicePixelRatio;
      const screen = [0, 0, innerWidth * d, innerHeight * d].map(Math.round);
      return { key: "preview", color: "#1c2238", position: "fill", monitor: screen, desktop: screen, bar: [r.left, r.top, r.width, r.height].map(v => Math.round(v * d)) };
    },
    wallpaper_image: () => wallpaperArt,
    set_volume({ pid, volume, muted }) {
      const target = pid == null ? audio : audio.apps.find(a => a.pid === pid);
      if (volume != null) target.volume = Math.round(volume);
      if (muted != null) target.muted = muted;
    },
    app_windows: ({ pid } = {}) => pid == null || pid === 812 ? windows
      : (running.find(a => a.pid === pid)?.windows ?? []).map((w, i) => ({ ...w, title: `${running.find(a => a.pid === pid).name}${i ? ` (${i + 1})` : ""}` })),
    launch_app: ({ key }) => log(`launch ${key}`),
    set_radio: ({ kind, on }) => { radios[kind] = on; },
    bluetooth_devices: () => [{ name: "AirPods Pro", connected: true }, { name: "MX Master 3S", connected: true }, { name: "Keychron K2", connected: false }],
    displays: () => [{ id: "built-in", name: "Built-in display", builtin: true, brightness: displayLevel }],
    set_brightness: ({ value }) => { displayLevel = value; },
    screens: () => [{ id: String.raw`\\.\DISPLAY1`, primary: true, rect: [0, 0, 2560, 1440] }]
      .concat(q.get("screens") === "2" ? [{ id: String.raw`\\.\DISPLAY2`, primary: false, rect: [2560, 0, 1920, 1080] }] : []),
    taskbar_can_hide: () => true,
    bar_info: () => ({ monitor: 1, bars: 1 }),
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
      try { return Promise.resolve(fn(args, label)); } catch (e) { return Promise.reject(String(e)); }
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

  // The README's animation (shots.ps1 -Demo): a pointer opens the track's menu and runs along
  // the bar through the others, the calendar changes palette under it, the bar turns to glass,
  // and the app menu opens. Timed from page load; demo.py steps the clock one frame at a time.
  if (q.get("demo")) {
    const pointer = $("pointer");
    const item = id => barFrame.contentDocument.querySelector(`[data-menu="${id}"]`);
    const moveTo = id => {
      const r = item(id).getBoundingClientRect();
      Object.assign(pointer.style, { left: `${r.left + r.width / 2}px`, top: `${r.top + r.height / 2 + 2}px` });
    };
    const fire = (id, type) => item(id).dispatchEvent(new PointerEvent(type, { button: 0 }));
    const look = patch => commands.save_settings({ settings: { ...settings, ...patch } });
    const steps = [
      [800, () => pointer.classList.add("on")],
      [850, () => moveTo("media")], [1200, () => fire("media", "pointerdown")],
      [1900, () => moveTo("stats")], [2050, () => fire("stats", "pointerenter")],
      [2600, () => moveTo("volume")], [2700, () => fire("volume", "pointerenter")],
      [3250, () => moveTo("bluetooth")], [3350, () => fire("bluetooth", "pointerenter")],
      [3900, () => moveTo("control")], [4000, () => fire("control", "pointerenter")],
      [4800, () => moveTo("clock")], [4950, () => fire("clock", "pointerenter")],
      [5600, () => look({ palette: "catppuccin" })],
      [6100, () => look({ palette: "nord", theme: "light" })],
      [6600, () => look({ palette: "rose-pine", theme: "dark" })],
      [7100, () => look({ palette: "gruvbox", theme: "light" })],
      [7600, () => look({ palette: "oriel", theme: "dark", glass: true })],
      [8300, () => moveTo("app")], [8650, () => fire("app", "pointerenter")],
      [9500, () => fire("app", "pointerdown")],
      // Back to how it started, so the animation loops without a jump.
      [9900, () => { pointer.classList.remove("on"); pointer.style.left = pointer.style.top = ""; look({ glass: false }); }],
    ];
    for (const [t, step] of steps) setTimeout(step, t);
  }

  if (q.get("open")) {
    setTimeout(() => {
      const item = barFrame.contentDocument.querySelector(`[data-menu="${q.get("open")}"]`);
      const { left, top } = item.getBoundingClientRect();
      commands.open_menu({ id: q.get("open"), item: { left, top }, toggle: false });
    }, 1500);
  }
})();
