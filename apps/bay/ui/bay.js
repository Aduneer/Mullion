"use strict";
// The overview on one screen. DWM draws each window's live thumbnail over this page, but a
// thumbnail takes no input: clicks and keys come here as usual, and the page knows where each
// thumbnail is (the backend sends the grid as "open"). So the page draws a ring around the window
// you're on and each window's icon and title below it, picks one on a click or Enter, and draws
// the backdrop: the wallpaper, blurred and dimmed. A window dragged from here is the backend's to
// carry: it follows the pointer onto another screen, where this page can't.
{
const T = window.__TAURI__;
const me = T.webviewWindow.getCurrentWebviewWindow();
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const listen = (event, fn) => me.listen(event, e => fn(e.payload));
const $ = id => document.getElementById(id);
const grid = $("grid"), canvas = $("paper");

// An overview, not a web page: no browser right-click menu, and none of its keys.
document.addEventListener("contextmenu", e => e.preventDefault());
document.addEventListener("keydown", e => {
  const k = e.key.toLowerCase();
  if (["f5", "f7", "f12", "browserback", "browserforward"].includes(k)
    || (e.ctrlKey && ["r", "f", "g", "p", "u", "j", "+", "-", "=", "0"].includes(k))
    || (e.ctrlKey && e.shiftKey && ["i"].includes(k))
    || (e.altKey && ["arrowleft", "arrowright"].includes(k))) e.preventDefault();
}, true);
document.addEventListener("wheel", e => { if (e.ctrlKey) e.preventDefault(); }, { passive: false });

const esc = s => String(s ?? "").replace(/[&<>"']/g, c => `&#${c.charCodeAt(0)};`);
const WINDOW_GLYPH = `<svg class="ic" viewBox="0 0 24 24" aria-hidden="true"><rect x="3.5" y="5" width="17" height="14" rx="2.5"/><path d="M3.5 9h17"/></svg>`;

// --- look ----------------------------------------------------------------------------------
let look = { theme: "auto", palette: "oriel", backdrop_kind: "wallpaper" };
const dark = matchMedia("(prefers-color-scheme: dark)");
function applyLook() {
  const root = document.documentElement;
  root.dataset.palette = look.palette;
  root.dataset.mode = look.theme === "auto" ? (dark.matches ? "dark" : "light") : look.theme;
  document.body.classList.toggle("plain", look.backdrop_kind === "plain");
}
dark.addEventListener("change", applyLook);

// --- the backdrop ----------------------------------------------------------------------------
// The screen's wallpaper as Windows fits it, drawn at a tenth of the size with a blur, then
// stretched back up: softer still, and drawn once per picture rather than every frame.
const SMALL = 10;
let paperUrl = null;

function drawBackdrop(b) {
  document.body.style.setProperty("--paper-color", b.color);
  if (!b.url) {
    paperUrl = null;
    canvas.getContext("2d").clearRect(0, 0, canvas.width, canvas.height);
    return;
  }
  if (b.url === paperUrl) return;
  const img = new Image();
  img.onload = () => { paperUrl = b.url; paint(img, b); };
  img.src = b.url;
}

/** Where Windows puts the picture, as rects on the screen (physical pixels). */
function placements(b, iw, ih) {
  const [l, t, r, bottom] = b.fit === "span" ? b.span : b.monitor;
  const mw = r - l, mh = bottom - t;
  const centred = k => [[l + (mw - iw * k) / 2, t + (mh - ih * k) / 2, iw * k, ih * k]];
  switch (b.fit) {
    case "fit": return centred(Math.min(mw / iw, mh / ih));
    case "stretch": return [[l, t, mw, mh]];
    case "center": return centred(1);
    case "tile": {
      const tiles = [];
      for (let y = t; y < bottom && tiles.length < 400; y += ih) for (let x = l; x < r; x += iw) tiles.push([x, y, iw, ih]);
      return tiles;
    }
    default: return centred(Math.max(mw / iw, mh / ih)); // fill, span
  }
}

function paint(img, b) {
  const [wl, wt, wr, wb] = b.work;
  canvas.width = Math.ceil((wr - wl) / SMALL);
  canvas.height = Math.ceil((wb - wt) / SMALL);
  const ctx = canvas.getContext("2d");
  ctx.fillStyle = b.color;
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  const rects = placements(b, img.naturalWidth, img.naturalHeight).map(([x, y, w, h]) => [(x - wl) / SMALL, (y - wt) / SMALL, w / SMALL, h / SMALL]);
  // Sharp first, so the blur's soft edge at the sides fades into the picture, not to black.
  for (const r of rects) ctx.drawImage(img, ...r);
  ctx.filter = "blur(3px)";
  for (const r of rects) ctx.drawImage(img, ...r);
  ctx.filter = "none";
}

// --- the windows ---------------------------------------------------------------------------------
let slots = [], selected = -1, open = false;

const pic = s => (s.icon ? `<img src="${esc(s.icon)}" alt="">` : WINDOW_GLYPH);

function draw(shown, settling = false) {
  slots = shown.slots;
  selected = -1;
  grid.innerHTML = slots.map((s, i) => {
    // A little wider than the window, into the gap, so a narrow one's title still reads.
    const lw = s.w + 32;
    return `<div class="slot${s.minimized ? " card" : ""}" data-i="${i}" style="left:${s.x}px;top:${s.y}px;width:${s.w}px;height:${s.h}px">${s.minimized ? `${pic(s)}<span>Minimized</span>` : ""}</div>`
      + `<div class="label" data-i="${i}" style="left:${s.x + s.w / 2 - lw / 2}px;top:${s.y + s.h}px;width:${lw}px"><span class="name">${pic(s)}<span>${esc(s.title)}</span></span></div>`;
  }).join("");
  $("empty").hidden = slots.length > 0;
  open = true;
  document.body.classList.remove("closing");
  // The windows fly to their new places: their labels wait for them.
  clearTimeout(settled);
  document.body.classList.toggle("settling", settling);
  if (settling) settled = setTimeout(() => document.body.classList.remove("settling"), shown.ms);
}
let settled = 0;

function select(i) {
  grid.querySelector(".slot.selected")?.classList.remove("selected");
  selected = i;
  grid.querySelector(`.slot[data-i="${i}"]`)?.classList.add("selected");
}

/** The windows in reading order: rows top to bottom, each left to right. */
const reading = () => slots.map((s, i) => i).sort((a, b) => (Math.round(slots[a].y) - Math.round(slots[b].y)) || (slots[a].x - slots[b].x));
/** The window that was in front, to start from. */
const front = () => slots.reduce((best, s, i) => (best < 0 || s.z < slots[best].z ? i : best), -1);

/** Tab and Shift+Tab: the next window in reading order, then on to the next screen. */
function step(dir) {
  if (!slots.length) return invoke("next_screen", { dir });
  const order = reading();
  if (selected < 0) return select(order[dir > 0 ? 0 : order.length - 1]);
  const at = order.indexOf(selected) + dir;
  if (at < 0 || at >= order.length) {
    select(-1);
    return invoke("next_screen", { dir });
  }
  select(order[at]);
}

/** An arrow: the nearest window that way. Left or right past the last, the next screen. */
function move(dx, dy) {
  if (selected < 0) return slots.length ? select(front()) : invoke("next_screen", { dir: dx || 1 });
  const c = s => [s.x + s.w / 2, s.y + s.h / 2];
  const [x0, y0] = c(slots[selected]);
  let best = -1, score = Infinity;
  slots.forEach((s, i) => {
    const [x, y] = c(s);
    const along = (x - x0) * dx + (y - y0) * dy;
    const across = Math.abs((x - x0) * dy) + Math.abs((y - y0) * dx);
    if (i === selected || along <= 1) return;
    if (along + 2 * across < score) { score = along + 2 * across; best = i; }
  });
  if (best >= 0) select(best);
  else if (dx) { select(-1); invoke("next_screen", { dir: dx }); }
}

function pick(i) {
  const s = slots[i];
  if (!s || !open) return;
  open = false;
  document.body.classList.add("closing");
  invoke("pick", { hwnd: s.hwnd, pid: s.pid }).catch(() => invoke("dismiss"));
}

function dismiss() {
  if (!open) return;
  open = false;
  invoke("dismiss");
}

// --- dragging to another screen ---------------------------------------------------------------
// Past a few pixels a press on a window is a drag, and the backend takes it from there; this page
// just stops drawing that window's ring and label, and says when the button's let go, which it
// hears even over another screen since it holds the mouse while it's down. "moved" brings the
// grid back.
let down = null, dragged = false;

grid.addEventListener("mousedown", e => {
  dragged = false;
  const t = e.target.closest(".slot");
  down = e.button === 0 && t && open && !slots[t.dataset.i].minimized ? { i: Number(t.dataset.i), x: e.clientX, y: e.clientY } : null;
});
document.addEventListener("mouseup", e => {
  down = null;
  if (dragged && e.button === 0) invoke("let_go");
});
// If the page loses the mouse some other way (another window taking the focus), the drag ends there.
window.addEventListener("blur", () => { if (dragged) invoke("let_go"); });
document.addEventListener("mousemove", e => {
  if (!down || dragged || !(e.buttons & 1) || Math.hypot(e.clientX - down.x, e.clientY - down.y) < 6) return;
  const s = slots[down.i];
  dragged = true;
  select(-1);
  for (const el of grid.querySelectorAll(`[data-i="${down.i}"]`)) el.classList.add("dragging");
  invoke("drag", { hwnd: s.hwnd, pid: s.pid, fx: (down.x - s.x) / s.w, fy: (down.y - s.y) / s.h })
    .catch(e => {
      for (const el of grid.querySelectorAll(".dragging")) el.classList.remove("dragging");
      if (e !== "not now") note(String(e));
    });
  down = null;
});

/** Why a window didn't move, for a few seconds. */
let noted = 0;
function note(text) {
  $("note").textContent = text;
  $("note").hidden = false;
  clearTimeout(noted);
  noted = setTimeout(() => { $("note").hidden = true; }, 4000);
}

grid.addEventListener("mousemove", e => {
  const t = e.target.closest("[data-i]");
  if (t && !dragged && Number(t.dataset.i) !== selected) select(Number(t.dataset.i));
});
document.addEventListener("click", e => {
  if (dragged) return;
  if (e.target.closest("#desktops")) {
    if (open) { open = false; invoke("desktops"); }
    return;
  }
  const t = e.target.closest("[data-i]");
  if (t) pick(Number(t.dataset.i));
  else dismiss();
});
document.addEventListener("keydown", e => {
  switch (e.key) {
    case "Escape": e.preventDefault(); return dismiss();
    case "Enter": case " ": e.preventDefault(); return pick(selected >= 0 ? selected : front());
    case "Tab": e.preventDefault(); return step(e.shiftKey ? -1 : 1);
    case "ArrowRight": e.preventDefault(); return move(1, 0);
    case "ArrowLeft": e.preventDefault(); return move(-1, 0);
    case "ArrowDown": e.preventDefault(); return move(0, 1);
    case "ArrowUp": e.preventDefault(); return move(0, -1);
  }
});

// --- from the backend ---------------------------------------------------------------------------
listen("open", shown => draw(shown));
listen("moved", ({ shown }) => { dragged = false; draw(shown, true); });
listen("note", note);
listen("close", () => { open = false; document.body.classList.add("closing"); });
listen("closed", () => {
  open = false;
  $("note").hidden = true;
  slots = [];
  selected = -1;
  grid.innerHTML = "";
  $("empty").hidden = true;
  document.body.classList.remove("closing");
});
// Tab from another screen's overview: start at this one's first (or last) window.
listen("enter", dir => {
  if (!slots.length) return;
  const order = reading();
  select(order[dir > 0 ? 0 : order.length - 1]);
});
listen("icon", ({ key, icon }) => {
  if (!icon) return;
  slots.forEach((s, i) => {
    if (s.icon_key !== key || s.icon) return;
    s.icon = icon;
    for (const el of grid.querySelectorAll(`[data-i="${i}"] .ic`)) el.outerHTML = `<img src="${esc(icon)}" alt="">`;
  });
});
listen("backdrop", drawBackdrop);
listen("settings", s => { look = { theme: s.theme, palette: s.palette, backdrop_kind: s.backdrop }; applyLook(); });

invoke("page").then(h => {
  look = h;
  applyLook();
  if (h.backdrop) drawBackdrop(h.backdrop);
  if (h.shown) draw(h.shown);
});
applyLook();
}
