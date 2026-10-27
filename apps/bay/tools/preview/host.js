// The preview's stand-in for the Rust backend and for DWM: a made-up desktop of windows, the
// overview's grid laid out as grid.rs does, and a copy of each window drawn over the page where
// its thumbnail would be, flying there from where the window is as the desktop fades.
//
// URL options, all optional:
//   theme=auto|dark|light  palette=nord  backdrop=plain  minimized=0 (leave them out)
//   state=open (the default) | closed | flight (part way, k=0.5)  sel=2 (→ that many times)
//   ms=250 (how long it flies)
"use strict";
(() => {
  const q = new URLSearchParams(location.search);
  const $ = id => document.getElementById(id);
  const desktop = $("desktop"), overview = $("overview"), frame = $("page"), thumbs = $("thumbs");
  const log = msg => { $("log").textContent = msg; clearTimeout(log.t); log.t = setTimeout(() => ($("log").textContent = ""), 4000); };
  const MS = Number(q.get("ms") || 250);

  // --- made-up pictures ------------------------------------------------------------------
  const data = svg => "data:image/svg+xml;base64," + btoa(svg);
  // Plain made-up app icons (no real apps' logos): a rounded square and a glyph.
  const appIcon = (from, to, glyph) => data(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs><rect x="4" y="4" width="56" height="56" rx="14" fill="url(#g)"/><g fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">${glyph}</g></svg>`);
  const ICONS = {
    notes: appIcon("#ffd36b", "#f0a92e", `<path d="M22 18h20v28H22zM27 26h10M27 32h10M27 38h6"/>`),
    mail: appIcon("#3d8bfd", "#1d5fd0", `<rect x="16" y="20" width="32" height="24" rx="3"/><path d="M16 22l16 12 16-12"/>`),
    code: appIcon("#7a8cff", "#4a5ae0", `<path d="M26 22l-10 10 10 10M38 22l10 10-10 10"/>`),
    photos: appIcon("#4fd1c5", "#2b8fa8", `<rect x="16" y="18" width="32" height="28" rx="4"/><path d="M18 42l9-10 7 7 4-4 8 7"/><circle cx="38" cy="26" r="3"/>`),
    music: appIcon("#4fd18a", "#1f9d57", `<path d="M26 40V22l16-4v18"/><circle cx="23" cy="40" r="3"/><circle cx="39" cy="36" r="3"/>`),
    terminal: appIcon("#4a4f5c", "#22252c", `<path d="M19 24l8 8-8 8M31 41h14"/>`),
    browser: appIcon("#5aa0f2", "#2f6fd1", `<circle cx="32" cy="32" r="15"/><path d="M17 32h30M32 17c-6 8-6 22 0 30M32 17c6 8 6 22 0 30"/>`),
    calendar: appIcon("#ff7a7a", "#d94848", `<rect x="18" y="20" width="28" height="26" rx="4"/><path d="M18 28h28M26 16v8M38 16v8"/>`),
  };
  const wallpaper = data(`<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2d5b8f"/><stop offset=".55" stop-color="#e88a6a"/><stop offset="1" stop-color="#f5c38a"/></linearGradient></defs><rect width="1600" height="900" fill="url(#g)"/><circle cx="1150" cy="470" r="120" fill="#ffe2a8" opacity=".9"/><path d="M0 560 Q300 500 620 560 T1250 540 T1600 560 V900 H0Z" fill="#1f4f7a"/><path d="M0 640 Q400 600 800 650 T1600 630 V900 H0Z" fill="#173b5c"/><path d="M0 560 L120 470 L220 520 L330 430 L470 540 L560 500 L620 560Z" fill="#26344d"/><path d="M1180 560 L1290 480 L1380 520 L1480 440 L1600 520 V560Z" fill="#26344d"/></svg>`);
  document.body.style.setProperty("--wallpaper", `url("${wallpaper}")`);

  // --- the made-up windows -------------------------------------------------------------------
  const lines = (...widths) => widths.map(w => `<div class="line" style="width:${w}%"></div>`).join("");
  const block = (h, from, to) => `<div class="block" style="height:${h}px;background:linear-gradient(135deg,${from},${to})"></div>`;
  // title, icon, program, place on the screen [x, y, w, h], dark, minimized, body
  const WINDOWS = [
    ["Tram 28 timetable - Browser", "browser", String.raw`C:\Program Files\Browser\browser.exe`, [90, 40, 860, 560], false, false,
      `${lines(40)}${block(180, "#ffd21f", "#f0a92e")}${lines(90, 84, 70, 88, 60)}`],
    ["main.rs - Code", "code", String.raw`C:\Program Files\Code\code.exe`, [700, 120, 820, 600], true, false,
      `<div class="mono"><span style="color:#c792ea">fn</span> <span style="color:#82aaff">main</span>() {\n    <span style="color:#c792ea">let</span> trams = timetable::<span style="color:#82aaff">load</span>(<span style="color:#c3e88d">"28"</span>);\n    <span style="color:#c792ea">for</span> t <span style="color:#c792ea">in</span> trams.<span style="color:#82aaff">next</span>(<span style="color:#f78c6c">3</span>) {\n        println!(<span style="color:#c3e88d">"{t}"</span>);\n    }\n}</div>`],
    ["Weekend away - Notes", "notes", String.raw`C:\Program Files\Notes\notes.exe`, [40, 430, 520, 380], false, false,
      `${lines(58, 86, 72, 64)}${block(90, "#ffb36b", "#e0577a")}${lines(40)}`],
    ["Inbox - Mail", "mail", String.raw`C:\Program Files\Mail\mail.exe`, [560, 360, 640, 440], false, false,
      `${lines(30)}${[0, 1, 2, 3, 4].map(i => `<div style="display:flex;gap:10px;align-items:center"><div style="width:28px;height:28px;border-radius:50%;background:hsl(${i * 70},55%,60%)"></div><div style="flex:1;display:grid;gap:6px">${lines(50 + i * 7, 80)}</div></div>`).join("")}`],
    ["Lisbon - Photos", "photos", String.raw`C:\Program Files\Photos\photos.exe`, [1060, 30, 500, 380], true, false,
      `<div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:6px">${["#ffd21f,#f0766b", "#8fd3ff,#3d63d8", "#f2d7b6,#c9774a", "#3b2a6b,#f0766b", "#4fd1c5,#2b8fa8", "#ffc76e,#e0577a"].map(c => block(90, ...c.split(","))).join("")}</div>`],
    ["Terminal", "terminal", String.raw`C:\Program Files\Terminal\terminal.exe`, [1150, 470, 420, 330], true, false,
      `<div class="mono" style="color:#9fe0a8">PS C:\\Users\\you&gt; cargo test\n   Compiling bay v0.3.1\n    Finished in 4.2s\n     Running 14 tests\n<span style="color:#ddd">test result: ok.</span></div>`],
    ["Now playing - Music", "music", String.raw`C:\Program Files\Music\music.exe`, [300, 200, 460, 300], true, true,
      `${block(120, "#4fd18a", "#1f6d97")}${lines(60, 40)}`],
    ["October - Calendar", "calendar", String.raw`C:\Program Files\Calendar\calendar.exe`, [500, 160, 600, 420], false, true, lines(50)],
  ].map(([title, icon, path, [x, y, w, h], dark, minimized, body], z) => ({ title, icon, path, x, y, w, h, dark, minimized, body, z, hwnd: 1000 + z * 4, pid: 4000 + z }));
  const shown = WINDOWS.filter(w => q.get("minimized") !== "0" || !w.minimized);

  const windowHtml = w => `<div class="caption"><img src="${ICONS[w.icon]}" alt=""><span>${w.title}</span><span>–□✕</span></div><div class="body">${w.body}</div>`;
  function makeWindow(w) {
    const el = document.createElement("div");
    el.className = `window${w.dark ? " dark" : ""}`;
    Object.assign(el.style, { width: `${w.w}px`, height: `${w.h}px` });
    el.innerHTML = windowHtml(w);
    return el;
  }
  // Top-most last in the page, so it draws on top; the minimized ones aren't on the screen.
  for (const w of [...WINDOWS].reverse().filter(w => !w.minimized)) {
    const el = makeWindow(w);
    Object.assign(el.style, { left: `${w.x}px`, top: `${w.y}px` });
    desktop.append(el);
  }
  $("taskbar").innerHTML = ["browser", "code", "notes", "mail", "photos", "terminal", "music", "calendar"].map(i => `<img src="${ICONS[i]}" alt="">`).join("");

  // --- the grid, as grid.rs lays it out ----------------------------------------------------------
  const MARGIN = 56, LABEL = 40, GAP = 40;
  function layout(wins, area) {
    const n = wins.length;
    const fit = (w, cw, ch) => Math.min(cw / w.w, ch / w.h, 1);
    let best = null;
    for (let cols = n; cols >= 1; cols--) {
      const rows = Math.ceil(n / cols);
      const cw = (area.w - GAP * (cols - 1)) / cols, ch = (area.h - GAP * (rows - 1)) / rows - LABEL;
      if (cw <= 1 || ch <= 1) continue;
      const shown = wins.reduce((s, w) => s + w.w * w.h * fit(w, cw, ch) ** 2, 0);
      if (!best || shown > best.shown) best = { shown, cols, cw, ch };
    }
    if (!best) return [];
    const { cols, cw, ch } = best;
    const rows = Math.ceil(n / cols), gridH = (ch + LABEL) * rows + GAP * (rows - 1);
    const order = wins.map((_, i) => i).sort((a, b) => (wins[a].y + wins[a].h / 2) - (wins[b].y + wins[b].h / 2));
    const out = [];
    for (let row = 0; row * cols < n; row++) {
      const chunk = order.slice(row * cols, row * cols + cols).sort((a, b) => (wins[a].x + wins[a].w / 2) - (wins[b].x + wins[b].w / 2));
      const rowW = cw * chunk.length + GAP * (chunk.length - 1);
      const x0 = area.x + (area.w - rowW) / 2, y0 = area.y + (area.h - gridH) / 2 + row * (ch + LABEL + GAP);
      chunk.forEach((at, i) => {
        const s = fit(wins[at], cw, ch), w = wins[at].w * s, h = wins[at].h * s;
        out[at] = { x: x0 + i * (cw + GAP) + (cw - w) / 2, y: y0 + (ch - h) / 2, w, h };
      });
    }
    return out;
  }

  // --- the overview ------------------------------------------------------------------------------
  const listeners = {};
  function listen(event, fn) { (listeners[event] ??= []).push(fn); return () => {}; }
  const emit = (event, payload) => { for (const fn of listeners[event] ?? []) fn(structuredClone(payload)); };
  let flights = [], desk = null, phase = "closed";

  /** Every copy `k` of the way from its window (0) to its place (1), and the desktop's faded. */
  function place(k) {
    desk.style.opacity = 1 - k;
    for (const f of flights) {
      const a = f.from, b = f.to, at = (p, r) => p + (r - p) * k;
      const w = at(a.w, b.w), h = at(a.h, b.h);
      f.el.style.opacity = f.fade ? k : 1;
      f.el.style.transform = `translate(${at(a.x, b.x)}px, ${at(a.y, b.y)}px) scale(${w / f.win.w}, ${h / f.win.h})`;
    }
  }
  const ease = t => 1 - (1 - t) ** 3;
  function animate(opening, done) {
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const start = performance.now();
    const step = now => {
      const t = reduce ? 1 : Math.min((now - start) / MS, 1);
      place(opening ? ease(t) : 1 - ease(t));
      if (t < 1) requestAnimationFrame(step); else done?.();
    };
    requestAnimationFrame(step);
  }

  function open() {
    const area = { x: MARGIN, y: MARGIN, w: overview.clientWidth - 2 * MARGIN, h: overview.clientHeight - 2 * MARGIN };
    const to = layout(shown, area);
    thumbs.innerHTML = "";
    desk = document.createElement("div");
    desk.className = "desk";
    thumbs.append(desk);
    // Bottom-most first, as Bay registers them, so the stack stays as it was.
    flights = shown.map((win, i) => ({ win, to: to[i], from: win, fade: win.minimized })).reverse().filter(f => !f.win.minimized);
    for (const f of flights) { f.el = makeWindow(f.win); thumbs.append(f.el); }
    const slots = shown.map((w, i) => ({ hwnd: w.hwnd, pid: w.pid, title: w.title, icon_key: w.path, icon: ICONS[w.icon], minimized: w.minimized, z: w.z, ...to[i] }));
    phase = "open";
    overview.classList.add("on");
    place(0);
    emit("open", { slots, ms: MS });
    if (q.get("state") === "flight") return place(0.5);
    animate(true);
  }

  function close(picked) {
    if (phase !== "open") return;
    phase = "closing";
    emit("close", null);
    const f = flights.find(f => f.win.hwnd === picked);
    if (f) thumbs.append(f.el); // on top
    animate(false, () => {
      overview.classList.remove("on");
      thumbs.innerHTML = "";
      phase = "closed";
      emit("closed", null);
      if (f) {
        // As land() does: the picked window on top, in front.
        const el = [...desktop.children].find(e => e.querySelector(".caption span").textContent === f.win.title);
        if (el) desktop.append(el);
      }
      log(picked ? `picked ${shown.find(w => w.hwnd === picked)?.title}` : "closed");
    });
  }

  const look = () => ({ theme: q.get("theme") || "dark", palette: q.get("palette") || "oriel", backdrop_kind: q.get("backdrop") || "wallpaper" });
  const backdrop = () => {
    const w = overview.clientWidth, h = overview.clientHeight, screen = [0, 0, w, h + 48];
    return { url: wallpaper, fit: "fill", color: "#2d5b8f", monitor: screen, work: [0, 0, w, h], span: screen, scale: 1 };
  };

  window.BayHost = {
    listen,
    invoke(cmd, args) {
      switch (cmd) {
        case "page": return Promise.resolve({ ...look(), backdrop: backdrop(), shown: null });
        case "pick": close(args.hwnd); return Promise.resolve();
        case "dismiss": close(null); return Promise.resolve();
        case "next_screen": log("on to the next screen (this preview has one, so back to this one)"); emit("enter", args.dir); return Promise.resolve();
        case "desktops": close(null); log("then Windows' Task View would open"); return Promise.resolve();
        // Bay carries a dragged window to another screen; this preview has one, so it stays.
        case "drag": log(`drag ${shown.find(w => w.hwnd === args.hwnd)?.title}: this preview has one screen`); return Promise.reject("one screen");
      }
      log(`${cmd}(${JSON.stringify(args)})`);
      return Promise.resolve(null);
    },
  };

  // Win+Ctrl+Up, as the real shortcut does.
  const toggle = () => (phase === "closed" ? open() : phase === "open" ? close(null) : null);
  document.addEventListener("keydown", e => { if (e.ctrlKey && e.metaKey && e.key === "ArrowUp") toggle(); });
  desktop.addEventListener("dblclick", toggle);

  frame.addEventListener("load", () => {
    setTimeout(() => {
      if (q.get("state") === "closed") return;
      open();
      frame.contentWindow.focus();
      const sel = Number(q.get("sel") || 0);
      for (let i = 0; i < sel; i++) frame.contentDocument.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    }, 100);
  });
  frame.src = "/ui/index.html";
})();
