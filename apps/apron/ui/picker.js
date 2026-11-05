"use strict";
// An app's windows, when it has several and its icon in the dock is clicked: pick one to bring it
// forward, or close one with its ×. Arrow keys and Enter work too; Escape, or a click anywhere
// else, puts the panel away.
{
const T = window.__TAURI__;
const me = T.webviewWindow?.getCurrentWebviewWindow?.();
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const listen = (event, fn) => (me ?? T.event).listen(event, e => fn(e.payload));
const $ = id => document.getElementById(id);
const root = document.documentElement;
const panel = $("panel"), list = $("list");

document.addEventListener("contextmenu", e => e.preventDefault());
document.addEventListener("wheel", e => { if (e.ctrlKey) e.preventDefault(); }, { passive: false });

const esc = s => String(s ?? "").replace(/[&<>"']/g, c => `&#${c.charCodeAt(0)};`);
const CLOSE = `<svg class="ic" viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/></svg>`;
const dark = matchMedia("(prefers-color-scheme: dark)");

let picking = null, sel = 0;

function look(p) {
  root.dataset.palette = p.palette;
  root.dataset.mode = p.theme === "auto" ? (dark.matches ? "dark" : "light") : p.theme;
  root.dataset.edge = p.edge;
}

function count() {
  const n = picking.windows.length;
  $("count").textContent = `${n} windows`;
}

function render() {
  const p = picking;
  look(p);
  $("icon").innerHTML = p.icon ? `<img src="${p.icon}" alt="">` : `<span class="initial">${esc((p.name || "?").slice(0, 1))}</span>`;
  $("name").textContent = p.name;
  count();
  list.innerHTML = p.windows.map((w, i) => `
    <div class="row${w.front ? " front" : ""}${w.minimized ? " minimized" : ""}" role="option" data-i="${i}" title="${esc(w.title)}">
      <span class="title">${esc(w.title || p.name)}</span>
      ${w.minimized ? `<span class="state">Minimized</span>` : ""}
      <button class="close" aria-label="Close this window" title="Close">${CLOSE}</button>
    </div>`).join("");
  sel = Math.max(0, p.windows.findIndex(w => w.front));
  mark();
}

function mark() {
  [...list.children].forEach((r, i) => r.classList.toggle("sel", i === sel && keyed));
}

// The selection shows only once the keys are used; the pointer has its own hover.
let keyed = false;

function pick(i) {
  const w = picking.windows[i];
  if (w) invoke("picker_pick", { hwnd: w.hwnd, pid: w.pid }).catch(err => console.warn(err));
}

function close(i) {
  const w = picking.windows[i];
  if (!w) return;
  invoke("picker_close", { hwnd: w.hwnd, pid: w.pid }).catch(err => console.warn(err));
  picking.windows.splice(i, 1);
  if (picking.windows.length < 2) return invoke("picker_hide", { back: false });
  list.children[i].remove();
  [...list.children].forEach((r, j) => { r.dataset.i = j; });
  sel = Math.min(sel, picking.windows.length - 1);
  count();
  mark();
  report();
}

list.addEventListener("click", e => {
  const row = e.target.closest(".row");
  if (!row) return;
  const i = Number(row.dataset.i);
  if (e.target.closest(".close")) return close(i);
  pick(i);
});
// A middle click closes, as on a tab.
list.addEventListener("auxclick", e => {
  const row = e.target.closest(".row");
  if (row && e.button === 1) close(Number(row.dataset.i));
});

document.addEventListener("keydown", e => {
  if (!picking) return;
  const n = picking.windows.length;
  if (e.key === "Escape") return invoke("picker_hide", { back: true });
  if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Tab") {
    e.preventDefault();
    keyed = true;
    const up = e.key === "ArrowUp" || (e.key === "Tab" && e.shiftKey);
    sel = (sel + (up ? n - 1 : 1)) % n;
    return mark();
  }
  if (e.key === "Enter" || e.key === " ") { e.preventDefault(); return pick(sel); }
  if (e.key === "Delete") { e.preventDefault(); return close(sel); }
});

/** Tell Apron how big the panel is, so it can size the window and put it by the dock. */
function report() {
  const r = document.body.getBoundingClientRect();
  return invoke("picker_show", { width: Math.ceil(r.width), height: Math.ceil(r.height) });
}

listen("pick", async p => {
  picking = p;
  keyed = false;
  panel.classList.remove("in");
  render();
  await report().catch(err => console.warn(err));
  requestAnimationFrame(() => panel.classList.add("in"));
});
// Put away: ready to slide in again next time.
listen("picker_hidden", () => panel.classList.remove("in"));
dark.addEventListener("change", () => picking && look(picking));
}
