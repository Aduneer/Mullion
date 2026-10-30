// The preview's stand-in for Windows and for Sash's mover: a made-up desktop of windows, moved
// with the same rules as layout.rs (sixths of the work area, halves that become two-thirds and a
// third when pressed again, a half and the other arrow making a quarter).
//
// URL options, all optional:
//   scene=split (the default: a half and two quarters) | thirds | free
//   mode=light (light windows, but the code editor)  gap=8  keys=ctrl+alt+left (show those keys)
//   note=1 (Sash's note at the bottom; notemode=light, text=… for what it says)
"use strict";
(() => {
  const q = new URLSearchParams(location.search);
  const $ = id => document.getElementById(id);
  const desktop = $("desktop");
  const GAP = Number(q.get("gap") || 0);

  // --- made-up pictures ------------------------------------------------------------------
  const data = svg => "data:image/svg+xml;base64," + btoa(svg);
  // Plain made-up app icons (no real apps' logos): a rounded square and a glyph.
  const appIcon = (from, to, glyph) => data(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs><rect x="4" y="4" width="56" height="56" rx="14" fill="url(#g)"/><g fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">${glyph}</g></svg>`);
  const ICONS = {
    code: appIcon("#7a8cff", "#4a5ae0", `<path d="M26 22l-10 10 10 10M38 22l10 10-10 10"/>`),
    browser: appIcon("#5aa0f2", "#2f6fd1", `<circle cx="32" cy="32" r="15"/><path d="M17 32h30M32 17c-6 8-6 22 0 30M32 17c6 8 6 22 0 30"/>`),
    terminal: appIcon("#4a4f5c", "#22252c", `<path d="M19 24l8 8-8 8M31 41h14"/>`),
    notes: appIcon("#ffd36b", "#f0a92e", `<path d="M22 18h20v28H22zM27 26h10M27 32h10M27 38h6"/>`),
  };
  const wallpaper = data(`<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4a3a6e"/><stop offset=".55" stop-color="#b86f8f"/><stop offset="1" stop-color="#f0b38a"/></linearGradient></defs><rect width="1600" height="900" fill="url(#g)"/><circle cx="420" cy="430" r="110" fill="#ffe2b8" opacity=".85"/><path d="M0 590 Q360 520 760 580 T1600 560 V900 H0Z" fill="#3d2f5c"/><path d="M0 680 Q500 630 900 690 T1600 670 V900 H0Z" fill="#2a2145"/></svg>`);
  document.body.style.setProperty("--wallpaper", `url("${wallpaper}")`);

  // --- the made-up windows -------------------------------------------------------------------
  const light = q.get("mode") === "light";
  const lines = (...widths) => widths.map(w => `<div class="line" style="width:${w}%"></div>`).join("");
  const block = (h, from, to) => `<div class="block" style="height:${h}px;background:linear-gradient(135deg,${from},${to})"></div>`;
  const FULL = [0, 6], A = [0, 3], B = [3, 6];
  // title, icon, dark, body, and where each scene has it: [h, v] spans of sixths, [x, y, w, h], or
  // nowhere.
  const WINDOWS = [
    ["timetable.rs - Code", "code", true,
      `<div class="mono"><span style="color:#c792ea">pub fn</span> <span style="color:#82aaff">next</span>(stop: &amp;Stop, n: <span style="color:#c792ea">usize</span>) -&gt; Vec&lt;Tram&gt; {\n    stop.trams\n        .<span style="color:#82aaff">iter</span>()\n        .<span style="color:#82aaff">filter</span>(|t| t.due &gt;= <span style="color:#82aaff">now</span>())\n        .<span style="color:#82aaff">take</span>(n)\n        .<span style="color:#82aaff">cloned</span>()\n        .<span style="color:#82aaff">collect</span>()\n}\n\n<span style="color:#676e95">// The 28 runs every 12 minutes.</span>\n<span style="color:#c792ea">const</span> EVERY: Duration = Duration::<span style="color:#82aaff">from_secs</span>(<span style="color:#f78c6c">720</span>);</div>`,
      { split: [A, FULL], thirds: [[2, 4], FULL], free: [200, 90, 760, 520] }],
    ["Tram 28 timetable - Browser", "browser", false,
      `${lines(40)}${block(150, "#ffd21f", "#f0a92e")}${lines(90, 84, 70, 88, 60)}`,
      { split: [B, A], thirds: [[4, 6], FULL], free: [640, 40, 820, 560] }],
    ["Terminal", "terminal", !light,
      `<div class="mono" style="color:${light ? "#2f7a3a" : "#9fe0a8"}">PS C:\\Users\\you\\timetable&gt; cargo test\n   Compiling timetable v0.2.0\n    Finished in 3.1s\n     Running 9 tests\n<span style="color:${light ? "#222" : "#ddd"}">test result: ok. 9 passed</span></div>`,
      { split: [B, B], free: [980, 470, 520, 340] }],
    ["Weekend away - Notes", "notes", false,
      `${lines(58, 86, 72, 64)}${block(90, "#ffb36b", "#e0577a")}${lines(40)}`,
      { thirds: [[0, 2], FULL], free: [60, 380, 520, 420] }],
  ].map(([title, icon, dark, body, at]) => ({ title, icon, dark, body, at }));
  const scene = ["split", "thirds", "free"].includes(q.get("scene")) ? q.get("scene") : "split";

  const work = () => ({ left: 0, top: 0, right: desktop.clientWidth, bottom: desktop.clientHeight });

  // --- layout.rs, in JavaScript ------------------------------------------------------------------
  function cut(a, b, [s, e], gap) {
    const at = k => a + Math.round((b - a) * k / 6);
    return [at(s) + (s === 0 ? gap : gap - Math.floor(gap / 2)), at(e) - (e === 6 ? gap : Math.floor(gap / 2))];
  }
  function rect(w, [h, v], gap) {
    const [left, right] = cut(w.left, w.right, h, gap), [top, bottom] = cut(w.top, w.bottom, v, gap);
    return { left, top, right, bottom };
  }
  const same = (a, b) => a[0] === b[0] && a[1] === b[1];
  const nextSize = ([s, e], start) => { const n = { 3: 4, 4: 2 }[e - s] ?? 3; return start ? [0, n] : [6 - n, 6]; };
  function step(place, dir) {
    const acrossWays = dir === "left" || dir === "right", toStart = dir === "left" || dir === "up";
    const half = toStart ? A : B;
    let along, across;
    if (!place) [along, across] = [half, FULL];
    else {
      [along, across] = acrossWays ? place : [place[1], place[0]];
      const here = !same(along, FULL) && (toStart ? along[0] === 0 : along[1] === 6);
      if (here && same(across, FULL)) along = nextSize(along, toStart);
      else if (here) across = FULL;
      else if (!same(across, FULL) && (same(along, FULL) || (toStart ? along[1] === 6 : along[0] === 0))) along = half;
      else [along, across] = [half, FULL];
    }
    return acrossWays ? [along, across] : [across, along];
  }

  // --- the screen ------------------------------------------------------------------------------
  // Front-most first.
  const wins = WINDOWS.filter(w => w.at[scene]).map(w => ({ ...w, el: null, place: null, box: null, before: null, zoomed: false }));
  function draw(w) {
    const r = w.zoomed ? work() : w.place ? rect(work(), w.place, GAP) : w.box;
    Object.assign(w.el.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.right - r.left}px`, height: `${r.bottom - r.top}px` });
    w.el.classList.toggle("zoomed", w.zoomed);
  }
  function front(w) {
    wins.splice(wins.indexOf(w), 1);
    wins.unshift(w);
    wins.forEach((x, i) => (x.el.style.zIndex = String(wins.length - i)));
    document.querySelectorAll("#taskbar img").forEach(img => img.classList.toggle("front", img.dataset.icon === w.icon));
  }
  $("taskbar").innerHTML = Object.keys(ICONS).map(i => `<img src="${ICONS[i]}" data-icon="${i}" alt="">`).join("");
  for (const w of wins) {
    const el = (w.el = document.createElement("div"));
    el.className = `window${w.dark ? " dark" : ""}`;
    el.innerHTML = `<div class="caption"><img src="${ICONS[w.icon]}" alt=""><span>${w.title}</span><span>–□✕</span></div><div class="body">${w.body}</div>`;
    const at = w.at[scene];
    if (at.length === 2) w.place = at;
    else w.box = { left: at[0], top: at[1], right: at[0] + at[2], bottom: at[1] + at[3] };
    el.addEventListener("mousedown", () => front(w));
    desktop.append(el);
  }
  const redraw = () => wins.forEach(draw);
  redraw();
  front(wins[0]);
  addEventListener("resize", redraw);

  // --- the keys ------------------------------------------------------------------------------
  const NAMES = { ctrl: "Ctrl", alt: "Alt", shift: "Shift", left: "←", right: "→", up: "↑", down: "↓", enter: "Enter", space: "Space", backspace: "Backspace" };
  const showKeys = spec => {
    $("keys").innerHTML = spec ? spec.split("+").map(k => `<kbd>${NAMES[k] ?? k}</kbd>`).join("<span>+</span>") : "";
  };
  showKeys(q.get("keys"));
  addEventListener("keydown", e => {
    if (!e.ctrlKey || !e.altKey) return;
    const key = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down", Enter: "enter", Space: "space", Backspace: "backspace" }[e.code];
    if (!key) return;
    e.preventDefault();
    const w = wins[0], now = w.place ? rect(work(), w.place, GAP) : w.box;
    if (key === "enter") w.zoomed = !w.zoomed;
    else if (key === "backspace") {
      if (w.before) [w.place, w.box, w.before] = [null, w.before, null];
      w.zoomed = false;
    } else {
      if (!w.before && !w.place) w.before = now;
      if (key === "space") {
        const wd = Math.min(now.right - now.left, work().right - 2 * GAP), ht = Math.min(now.bottom - now.top, work().bottom - 2 * GAP);
        const left = Math.round((work().right - wd) / 2), top = Math.round((work().bottom - ht) / 2);
        [w.place, w.box] = [null, { left, top, right: left + wd, bottom: top + ht }];
      } else w.place = step(w.zoomed ? [FULL, FULL] : w.place, key);
      w.zoomed = false;
    }
    draw(w);
    showKeys(`ctrl+alt+${key}`);
    clearTimeout(showKeys.t);
    showKeys.t = setTimeout(() => showKeys(""), 1400);
  });

  // --- Sash's note -----------------------------------------------------------------------------
  if (q.get("note") === "1") {
    const note = $("note");
    addEventListener("message", e => {
      if (!e.data?.note) return;
      Object.assign(note.style, { width: `${e.data.note.w}px`, height: `${e.data.note.h}px`, visibility: "visible" });
    });
    const p = new URLSearchParams({ mode: q.get("notemode") || "dark" });
    if (q.get("text")) p.set("text", q.get("text"));
    note.src = `/ui/note.html?${p}`;
  }
})();
