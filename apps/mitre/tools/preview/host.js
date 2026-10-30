// The preview's stand-in for Mitre's corners: the same zone, dwell and glow as watch.rs and
// glow.rs, on a made-up desktop.
//
// URL options, all optional:
//   corner=tl|tr|bl|br (show that corner's glow at its fullest, and its card, for a screenshot)
//   palette=nord (the glow's colour)  dwell=150
"use strict";
(() => {
  const q = new URLSearchParams(location.search);
  const $ = id => document.getElementById(id);
  const data = svg => "data:image/svg+xml;base64," + btoa(svg);
  const wallpaper = data(`<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2d3e66"/><stop offset=".55" stop-color="#6c79a8"/><stop offset="1" stop-color="#d6b49a"/></linearGradient></defs><rect width="1600" height="900" fill="url(#g)"/><circle cx="1180" cy="420" r="120" fill="#fbe3c4" opacity=".8"/><path d="M0 600 Q420 540 820 600 T1600 580 V900 H0Z" fill="#283a5e"/><path d="M0 700 Q520 650 940 710 T1600 690 V900 H0Z" fill="#1d2a47"/></svg>`);
  document.body.style.setProperty("--wallpaper", `url("${wallpaper}")`);

  // glow.rs, in JavaScript.
  const ACCENTS = { oriel: [0x39, 0x87, 0xe5], catppuccin: [0xcb, 0xa6, 0xf7], nord: [0x88, 0xc0, 0xd0], gruvbox: [0x8e, 0xc0, 0x7c], "rose-pine": [0xc4, 0xa7, 0xe7], solarized: [0x2a, 0xa1, 0x98] };
  const color = (ACCENTS[q.get("palette")] ?? ACCENTS.oriel).map(c => c + Math.floor((255 - c) / 2));
  const REACH = 84, FLASH_REACH = 1.3, STRENGTH = 0.8, FLASH_MS = 260, FADE_MS = 140, ZONE = 4;
  const DWELL = Number(q.get("dwell") || 150);
  const WHAT = { tl: ["Every window", "Bay"], tr: ["Task View", "Windows"], bl: ["Desktop", "Windows"], br: ["Apps", "Fanlight"] };
  const canvas = $("glow"), g = canvas.getContext("2d"), card = $("card");
  const side = Math.ceil(REACH * FLASH_REACH * devicePixelRatio);
  canvas.width = canvas.height = side;
  canvas.style.width = canvas.style.height = `${side / devicePixelRatio}px`;
  const ease = t => 1 - (1 - Math.min(Math.max(t, 0), 1)) ** 3;

  let phase = null, since = 0, corner = null, from = 0;
  function level() {
    const ms = performance.now() - since;
    if (phase === "grow") { const t = ease(ms / DWELL); return [0.35 + 0.65 * t, STRENGTH * t]; }
    if (phase === "flash") { const t = ms / FLASH_MS; return t < 1 ? [1 + (FLASH_REACH - 1) * ease(t), Math.min(STRENGTH + 0.2, 1) * (1 - t)] : null; }
    if (phase === "fade") { const t = ms / FADE_MS; return t < 1 ? [0.35 + 0.65 * from, STRENGTH * from * (1 - t)] : null; }
    if (phase === "still") return [1, STRENGTH];
    return null;
  }
  function draw() {
    const l = level();
    if (!l) { phase = null; g.clearRect(0, 0, side, side); return; }
    const [reach, strength] = l, radius = reach * side / FLASH_REACH;
    const img = g.createImageData(side, side);
    for (let y = 0; y < side; y++) for (let x = 0; x < side; x++) {
      const dx = (corner[1] === "l" ? x : side - 1 - x) + 0.5, dy = (corner[0] === "t" ? y : side - 1 - y) + 0.5;
      const d = Math.hypot(dx, dy) / radius, a = d >= 1 ? 0 : strength * (1 - d) ** 1.3, i = (y * side + x) * 4;
      img.data.set([...color, Math.round(a * 255)], i);
    }
    g.putImageData(img, 0, 0);
    requestAnimationFrame(draw);
  }
  function place(c) {
    corner = c;
    Object.assign(canvas.style, { left: c[1] === "l" ? "0" : "", right: c[1] === "r" ? "0" : "", top: c[0] === "t" ? "0" : "", bottom: c[0] === "b" ? "0" : "" });
    const [what, who] = WHAT[c];
    card.innerHTML = `${what}<small>${who}</small>`;
    Object.assign(card.style, { left: c[1] === "l" ? "96px" : "", right: c[1] === "r" ? "96px" : "", top: c[0] === "t" ? "96px" : "", bottom: c[0] === "b" ? "96px" : "" });
  }
  function start(next, c) {
    if (c) place(c);
    if (next === "fade") from = Math.min((performance.now() - since) / DWELL, 1);
    const idle = !phase;
    phase = next;
    since = performance.now();
    if (idle) requestAnimationFrame(draw);
  }

  // The pointer: entering a corner starts the dwell; leaving fades it.
  let visit = null, timer = 0;
  addEventListener("mousemove", e => {
    const [w, h] = [innerWidth, innerHeight];
    const v = (e.clientY < ZONE ? "t" : e.clientY >= h - ZONE ? "b" : "") + (e.clientX < ZONE ? "l" : e.clientX >= w - ZONE ? "r" : "");
    const c = v.length === 2 ? v : null;
    if (c === visit) return;
    clearTimeout(timer);
    if (visit && phase === "grow") start("fade");
    visit = c;
    card.classList.remove("on");
    if (!c || e.buttons) return;
    start("grow", c);
    timer = setTimeout(() => { start("flash"); card.classList.add("on"); }, DWELL);
  });

  if (WHAT[q.get("corner")]) {
    start("still", q.get("corner"));
    card.classList.add("on");
  }
})();
