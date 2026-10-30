// The preview's stand-in for Windows around Sill's dock: it puts the dock's frame where dock.rs
// puts its window (the whole strip along the edge, or just the shelf floating above the taskbar),
// and logs what the dock asks for.
//
// URL options: those mock.js reads (edge, size, float, glass, mode, palette, …), and
//   hide=1 (floating, start with the dock hidden, as it is until the pointer is at the edge)
"use strict";
(() => {
  const q = new URLSearchParams(location.search);
  const $ = id => document.getElementById(id);
  const frame = $("dock");
  const edge = q.get("edge") || "bottom", float = q.get("float") === "1";
  const icon = { small: 32, medium: 40, large: 52 }[q.get("size")] ?? 40;
  const thickness = icon + 26, taskbar = 48, gap = 6;
  const light = q.get("mode") === "light" || (!q.get("mode") && matchMedia("(prefers-color-scheme: light)").matches);
  document.body.classList.toggle("light", light);
  document.body.classList.toggle("float", float);

  // A made-up wallpaper with some detail, so the glass has something to blur.
  const wallpaperArt = "data:image/svg+xml;base64," + btoa(`<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2d3e66"/><stop offset=".55" stop-color="#6c79a8"/><stop offset="1" stop-color="#d6b49a"/></linearGradient></defs><rect width="1600" height="900" fill="url(#g)"/><circle cx="1180" cy="420" r="120" fill="#fbe3c4" opacity=".8"/><path d="M0 600 Q420 540 820 600 T1600 580 V900 H0Z" fill="#283a5e"/><path d="M0 760 Q520 700 940 770 T1600 740 V900 H0Z" fill="#1d2a47"/><circle cx="380" cy="820" r="70" fill="#e8a66b" opacity=".7"/><circle cx="1240" cy="840" r="90" fill="#7fc0d8" opacity=".6"/></svg>`);
  document.body.style.setProperty("--wallpaper", `url("${wallpaperArt}")`);

  // In the taskbar's place: the whole strip along the edge.
  function strip() {
    const s = frame.style;
    if (edge === "left") Object.assign(s, { left: "0", top: "0", width: `${thickness}px`, height: "100%" });
    else if (edge === "right") Object.assign(s, { left: `${innerWidth - thickness}px`, top: "0", width: `${thickness}px`, height: "100%" });
    else Object.assign(s, { left: "0", top: `${innerHeight - thickness}px`, width: "100%", height: `${thickness}px` });
  }
  // Floating: the shelf's size, centred along the edge, above the taskbar.
  let shelf = [0, 0];
  function floating() {
    const [w, h] = shelf, bottom = innerHeight - taskbar;
    const [x, y] = edge === "left" ? [gap, (bottom - h) / 2] : edge === "right" ? [innerWidth - gap - w, (bottom - h) / 2] : [(innerWidth - w) / 2, bottom - gap - h];
    Object.assign(frame.style, { left: `${x}px`, top: `${y}px`, width: `${w}px`, height: `${h}px` });
  }
  const place = () => (float ? floating() : strip());
  addEventListener("resize", place);

  const logEl = $("log");
  let logTimer = 0;
  window.SillHost = {
    wallpaperArt,
    log(text) {
      logEl.textContent = text;
      clearTimeout(logTimer);
      logTimer = setTimeout(() => { logEl.textContent = ""; }, 3000);
    },
    shelf(w, h) {
      shelf = [w, h];
      place();
    },
    // The harness page's own background stands in for the wallpaper behind the dock.
    wallpaper() {
      const r = frame.getBoundingClientRect(), d = devicePixelRatio;
      const screen = [0, 0, innerWidth * d, innerHeight * d].map(Math.round);
      return { key: "preview", color: "#27375a", position: "fill", monitor: screen, desktop: screen, bar: [r.left, r.top, r.width, r.height].map(v => Math.round(v * d)) };
    },
  };
  place();
  frame.src = "/ui/dock.html";
  // Floating, the dock shows once the pointer is at the edge: here, at once unless ?hide=1.
  if (float && q.get("hide") !== "1") frame.addEventListener("load", () => setTimeout(() => frame.contentWindow.SillMock.emit("slide", true), 300));
})();
