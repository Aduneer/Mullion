"use strict";
// The panel: the search field, the results as the backend groups them, and the settings view.
// Each keystroke asks the backend to search (numbered, so a slow answer to an older query is
// dropped), and icons come one by one as the shell draws them.
{
const T = window.__TAURI__;
const me = T.webviewWindow.getCurrentWebviewWindow();
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const listen = (event, fn) => me.listen(event, e => fn(e.payload));
const $ = id => document.getElementById(id);
const panel = $("panel"), input = $("q"), list = $("results"), setv = $("settings"), foot = $("foot"), pv = $("preview"), cats = $("cats");

// A panel, not a web page: no browser right-click menu, and none of its keys.
document.addEventListener("contextmenu", e => { if (e.target !== input && !e.target.classList.contains("text-in")) e.preventDefault(); });
document.addEventListener("keydown", e => {
  const k = e.key.toLowerCase();
  if (["f5", "f7", "f12", "browserback", "browserforward"].includes(k)
    || (e.ctrlKey && ["r", "f", "g", "p", "u", "j", "+", "-", "=", "0"].includes(k))
    || (e.ctrlKey && e.shiftKey && ["i"].includes(k))
    || (e.altKey && ["arrowleft", "arrowright"].includes(k))) e.preventDefault();
}, true);
document.addEventListener("wheel", e => { if (e.ctrlKey) e.preventDefault(); }, { passive: false });

const esc = s => String(s ?? "").replace(/[&<>"']/g, c => `&#${c.charCodeAt(0)};`);

// --- icons ------------------------------------------------------------------------------
// 24-unit boxes, drawn with currentColor so they follow the theme; stand-ins for results with
// no icon of their own, and while the shell draws those that have one.
const svg = body => `<svg class="ic" viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;
const GLYPHS = {
  lens: svg(`<circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5l5 5"/>`),
  app: svg(`<rect x="4" y="4" width="16" height="16" rx="4"/><path d="M4 9h16"/>`),
  file: svg(`<path d="M6.5 3.5h7l4 4v13h-11z"/><path d="M13.5 3.5v4h4"/>`),
  folder: svg(`<path d="M3.5 6.5a1.5 1.5 0 0 1 1.5-1.5h4.5l2 2h7.5a1.5 1.5 0 0 1 1.5 1.5v9a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5z"/>`),
  page: svg(`<circle cx="12" cy="12" r="3"/><path d="M12 2.8v2.4M12 18.8v2.4M2.8 12h2.4M18.8 12h2.4M5.5 5.5l1.7 1.7M16.8 16.8l1.7 1.7M5.5 18.5l1.7-1.7M16.8 7.2l1.7-1.7"/>`),
  window: svg(`<rect x="3.5" y="5" width="17" height="14" rx="2.5"/><path d="M3.5 9h17"/>`),
  clip: svg(`<rect x="5.5" y="4.5" width="13" height="16" rx="2"/><path d="M9 4.5V3.5h6v1M9 10h6M9 13.5h6M9 17h3.5"/>`),
  calc: svg(`<rect x="5" y="3.5" width="14" height="17" rx="2.5"/><path d="M8 7.5h8M8.5 12h1M11.5 12h1M14.5 12h1M8.5 16h1M11.5 16h1M14.5 16h1"/>`),
  web: svg(`<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c-3 3-3 14 0 17M12 3.5c3 3 3 14 0 17"/>`),
  url: svg(`<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>`),
  x: svg(`<path d="M6 6l12 12M18 6L6 18"/>`),
  action: svg(`<path d="M13.5 3.5L5.5 13.5h6l-1 7 8-10h-6z"/>`),
};
const COMMAND_GLYPHS = {
  lock: svg(`<rect x="5" y="10.5" width="14" height="10" rx="2"/><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5"/>`),
  sleep: svg(`<path d="M19.5 14.5A8 8 0 0 1 9.5 4.5a8 8 0 1 0 10 10z"/>`),
  signout: svg(`<path d="M14 4.5H6.5a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2H14M10 12h10M17 8.5l3.5 3.5-3.5 3.5"/>`),
  restart: svg(`<path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4h-4"/>`),
  shutdown: svg(`<path d="M12 3.5v8M7 6.3a7.5 7.5 0 1 0 10 0"/>`),
  desktop: svg(`<rect x="3.5" y="4.5" width="17" height="12" rx="2"/><path d="M9 20h6M12 16.5V20"/>`),
  screenshot: svg(`<path d="M7 3.5v13.5h13.5M3.5 7H17v13.5"/>`),
  emoji: svg(`<circle cx="12" cy="12" r="8.5"/><path d="M8.5 14a4 4 0 0 0 7 0"/><circle cx="9" cy="10" r="1" class="fill"/><circle cx="15" cy="10" r="1" class="fill"/>`),
  clipboard: GLYPHS.clip,
  "clear-clipboard": svg(`<path d="M5 7h14M9.5 7V5h5v2M7 7l1 13h8l1-13"/>`),
  settings: GLYPHS.page,
  reindex: svg(`<path d="M4.5 12a7.5 7.5 0 0 1 13-5.1M19.5 12a7.5 7.5 0 0 1-13 5.1M17.5 3.5v3.4h-3.4M6.5 20.5v-3.4h3.4"/>`),
  quit: svg(`<path d="M6 6l12 12M18 6L6 18"/>`),
};
$("lens").innerHTML = GLYPHS.lens;

/** Images the shell made, by icon key, so each is asked for once. */
const icons = new Map();
function glyph(hit) {
  if (hit.kind === "command") return COMMAND_GLYPHS[hit.key.slice(4)] ?? GLYPHS.app;
  return GLYPHS[hit.kind] ?? GLYPHS.file;
}
function pic(hit) {
  const url = hit.icon && icons.get(hit.icon);
  const thumb = hit.icon?.startsWith("thumb:") ? " thumb" : "";
  return `<span class="pic${thumb}" data-icon="${esc(hit.icon ?? "")}">${url ? `<img src="${esc(url)}" alt="">` : glyph(hit)}</span>`;
}
// Only for what's on screen, or nearly: a grid of every app would otherwise ask for a few hundred
// at once, and the shell draws them one at a time.
const iconWatch = new IntersectionObserver(entries => {
  for (const e of entries) {
    if (!e.isIntersecting) continue;
    iconWatch.unobserve(e.target);
    fetchIcon(e.target.dataset.icon);
  }
}, { root: list, rootMargin: "240px" });
function loadIcons() {
  iconWatch.disconnect();
  for (const el of list.querySelectorAll(".pic[data-icon]")) {
    if (el.dataset.icon && !icons.has(el.dataset.icon)) iconWatch.observe(el);
  }
}
function fetchIcon(key) {
  if (icons.has(key)) return;
  icons.set(key, null);
  invoke("icon", { key }).then(url => {
    icons.set(key, url);
    if (!url) return;
    for (const p of list.querySelectorAll(".pic[data-icon]")) {
      if (p.dataset.icon === key) p.innerHTML = `<img src="${esc(url)}" alt="">`;
    }
  }).catch(() => {});
}

// --- state ------------------------------------------------------------------------------
let hello = null, settings = null;
/** The rows on screen, in order (the Top Hit first), and which is selected. */
let rows = [], sel = 0;
/** Where the apps' grid is among the rows, when the Apps category shows one: [start, end). */
let grid = null;
/** A row waiting for its second Enter ("Restart"), by key. */
let asking = null;
let seq = 0, shown = 0;
/** Earlier searches, newest first (from the backend each time the panel opens), and which one is
 *  in the field while stepping through them with ↑ (-1: none). */
let past = [], back = -1;
let view = "search";
/** The category the search keeps to (a button in the field, Ctrl+1–4), or null for everything. */
let only = null;
/** The preview pane is open (Space); and the selection was moved with the keys since typing, so
 *  that Space previews rather than types a space. */
let previewing = false, moved = false;

// --- theme --------------------------------------------------------------------------------
const dark = matchMedia("(prefers-color-scheme: dark)");
function applyTheme() {
  const root = document.documentElement;
  root.dataset.palette = settings.palette;
  root.dataset.glass = settings.glass ? "on" : "off";
  root.dataset.mode = settings.theme === "auto" ? (dark.matches ? "dark" : "light") : settings.theme;
}
dark.addEventListener("change", () => settings && applyTheme());

// --- searching ----------------------------------------------------------------------------
let timer = 0;
input.addEventListener("input", () => {
  asking = null;
  back = -1;
  moved = false;
  clearTimeout(timer);
  // The first keystroke right away; then a moment's pause, while typing quickly, is enough.
  const text = input.value;
  timer = setTimeout(() => run(text), text.length <= 1 ? 0 : 35);
});

async function run(text) {
  const mine = ++seq;
  if (!text.trim() && !only) {
    seq++;
    render(null);
    return;
  }
  const [n, results] = await invoke("search", { text, seq: mine, only }).catch(e => (console.warn(e), [mine, null]));
  if (n !== seq || view !== "search") return; // typed past it
  shown = n;
  render(results);
}

function render(results) {
  rows = [];
  grid = null;
  if (!results) {
    list.innerHTML = "";
    foot.hidden = true;
    if (previewing) setPreview(false);
    return;
  }
  let html = "";
  // The Apps category as a grid, as Launchpad has them: the best match is just the first tile, and
  // with nothing typed, they're in alphabetical order.
  const gridded = only === "apps" && settings?.apps_view !== "list";
  if (gridded) {
    let apps = results.sections.find(s => s.hits[0]?.kind === "app");
    if (results.top?.kind === "app") {
      if (!apps) results.sections.unshift(apps = { title: "Applications", hits: [] });
      apps.hits.unshift(results.top);
      delete results.top;
    }
    if (apps && !input.value.trim()) apps.hits.sort((a, b) => a.title.localeCompare(b.title));
  }
  if (results.top) {
    rows.push(results.top);
    html += `<div class="top" role="group" aria-label="Top Hit">${row(results.top, 0, true)}</div>`;
  }
  for (const s of results.sections) {
    html += `<div class="section" role="group" aria-label="${esc(s.title)}"><h3>${esc(s.title)}</h3>`;
    if (gridded && s.hits[0]?.kind === "app") {
      grid = [rows.length, rows.length + s.hits.length];
      html += `<div class="grid">`;
      for (const h of s.hits) {
        rows.push(h);
        html += tile(h, rows.length - 1);
      }
      html += `</div></div>`;
      continue;
    }
    for (const h of s.hits) {
      rows.push(h);
      html += row(h, rows.length - 1, false);
    }
    if (!s.hits.length) html += `<div class="empty">Copy some text and it shows up here.</div>`;
    html += `</div>`;
  }
  list.innerHTML = html;
  sel = Math.min(sel, Math.max(0, rows.length - 1));
  if (!rows.length) {
    const blank = !input.value.trim();
    const why = only === "files" && blank ? "Type to search your files and folders"
      : only === "clipboard" && blank ? "Copy some text and it shows up here." : "No results";
    list.innerHTML = `<div class="empty">${why}</div>`;
  }
  select(0, false);
  loadIcons();
}

function row(h, i, top) {
  const cls = ["row", h.kind === "calc" && top ? "answer" : "", h.kind === "clip" ? "clip" : ""].filter(Boolean).join(" ");
  return `<button class="${cls}" role="option" data-i="${i}" tabindex="-1">${pic(h)}<span class="text"><span class="title">${esc(h.title)}</span><span class="sub">${esc(h.subtitle)}</span></span></button>`;
}

function tile(h, i) {
  return `<button class="tile" role="option" data-i="${i}" tabindex="-1" title="${esc(h.title)}">${pic(h)}<span class="title">${esc(h.title)}</span></button>`;
}

const inGrid = () => grid && sel >= grid[0] && sel < grid[1];
/** ↑ and ↓: a line at a time in the grid, and out of it at its edges. */
function step(down) {
  if (!inGrid()) return select(sel + (down ? 1 : -1));
  const [start, end] = grid, cols = previewing ? 4 : 6;
  const line = i => Math.floor((i - start) / cols);
  if (down) {
    const to = sel + cols;
    // Onto the short last line if there is one below, else out of the grid.
    return select(to < end ? to : line(sel) < line(end - 1) ? end - 1 : end);
  }
  select(sel - cols >= start ? sel - cols : start - 1);
}

function select(i, scroll = true) {
  if (!rows.length) {
    if (previewing) setPreview(false);
    return foot.hidden = true;
  }
  sel = (i + rows.length) % rows.length;
  if (asking && rows[sel].key !== asking) unask();
  for (const el of list.querySelectorAll("[data-i]")) {
    const on = Number(el.dataset.i) === sel;
    el.classList.toggle("sel", on);
    el.setAttribute("aria-selected", on);
    if (on && scroll) el.scrollIntoView({ block: "nearest" });
  }
  keys();
  if (previewing) loadPreview();
}

/** The keys the selected result takes, in the foot. */
function keys() {
  const h = rows[sel];
  if (!h) return foot.hidden = true;
  const k = (key, what) => `<span><b>${key}</b>${what}</span>`;
  const enter = { calc: "Copy", clip: "Paste", web: "Search", url: "Open", window: "Switch to", command: "Run" }[h.kind] ?? "Open";
  let html = k("↵", enter);
  if (h.kind === "file" || h.kind === "folder") html += k("Ctrl ↵", "Show in folder") + k("Ctrl C", "Copy path");
  if (h.kind === "app") html += k("Ctrl Shift ↵", "Run as administrator");
  if (h.kind === "clip") html += k("Ctrl ↵", "Copy only") + k("Del", "Forget");
  if (h.kind === "url" || h.kind === "web") html += k("Ctrl C", "Copy");
  html += previewing ? k("Esc", "Close preview") : k(moved ? "Space" : "Shift Space", "Preview");
  $("keys").innerHTML = html;
  foot.hidden = false;
}

let noteTimer = 0;
function note(text, bad = false) {
  const el = $("note");
  el.textContent = text;
  el.style.color = bad ? "var(--crit)" : "";
  foot.hidden = false;
  clearTimeout(noteTimer);
  noteTimer = setTimeout(() => { el.textContent = ""; if (!rows.length) foot.hidden = true; }, 2600);
}

function ask(h) {
  asking = h.key;
  const el = list.querySelector(`.row[data-i="${sel}"]`);
  if (!el) return;
  el.classList.add("ask");
  el.querySelector(".sub").textContent = `Press Enter again to ${h.title.toLowerCase()}`;
}
function unask() {
  asking = null;
  for (const el of list.querySelectorAll(".row.ask")) {
    el.classList.remove("ask");
    el.querySelector(".sub").textContent = rows[Number(el.dataset.i)]?.subtitle ?? "";
  }
}

async function choose(how = "open") {
  const h = rows[sel];
  if (!h) return;
  if (h.confirm && how === "open" && asking !== h.key) return ask(h);
  asking = null;
  if (h.kind === "clip" && how === "open") how = "paste";
  try {
    const next = await invoke("open", { key: h.key, how, query: input.value });
    if (next === "done") return;
    if (next.query !== undefined) {
      input.value = next.query;
      input.dispatchEvent(new Event("input"));
    } else if (next.view === "settings") {
      showSettings();
    } else if (next.note) {
      note(next.note);
    }
  } catch (e) {
    note(String(e), true);
  }
}

// --- preview ------------------------------------------------------------------------------------
// The selected result, larger, beside the list: a picture, the start of a text file, what's in a
// folder, a clipboard entry in full, and the facts about it. It follows the selection.

/** Large images by key, for going back and forth through the same few results. */
const bigs = new Map();
let pseq = 0, pvTimer = 0;
const when = secs => new Date(secs * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

function setPreview(on) {
  previewing = on;
  panel.classList.toggle("wide", on);
  pv.hidden = !on;
  if (on) loadPreview();
  else { pseq++; pv.innerHTML = ""; }
  fitNow();
  if (rows[sel]) keys();
}

/** A moment after the selection stops, so holding an arrow down doesn't read every file it passes. */
function loadPreview() {
  clearTimeout(pvTimer);
  pvTimer = setTimeout(drawPreview, pv.innerHTML ? 40 : 0);
}

async function drawPreview() {
  const h = rows[sel];
  if (!h || !previewing) return;
  const mine = ++pseq;
  const p = await invoke("preview", { key: h.key }).catch(() => null);
  if (mine !== pseq || !previewing) return;
  pv.innerHTML = previewHtml(h, p);
  pv.scrollTop = 0;
  const key = p?.image;
  if (!key) return;
  let url = bigs.get(key);
  if (url === undefined) {
    url = await invoke("icon", { key }).catch(() => null);
    if (bigs.size > 40) bigs.clear();
    bigs.set(key, url);
  }
  const el = pv.querySelector(".pv-pic");
  if (mine === pseq && url && el) el.innerHTML = `<img src="${esc(url)}" alt="">`;
}

function previewHtml(h, p) {
  // The list's icon stands in until the large one comes.
  const small = h.icon && icons.get(h.icon);
  const pic = small ? `<img src="${esc(small)}" alt="">` : glyph(h);
  const title = h.kind === "clip" ? "Copied text" : h.kind === "calc" ? h.subtitle : h.title;
  const sub = h.kind === "clip" || h.kind === "calc" ? (h.kind === "clip" ? h.subtitle : "") : h.subtitle;
  let html = p?.picture ? `<div class="pv-pic shot">${pic}</div>` : "";
  html += `<div class="pv-head">${p?.picture ? "" : `<div class="pv-pic">${pic}</div>`}<div class="pv-name"><div class="pv-title">${esc(title)}</div>${sub ? `<div class="pv-kind">${esc(sub)}</div>` : ""}</div></div>`;
  if (!p) return html + `<div class="empty">Nothing more to show</div>`;
  if (h.kind === "calc") html += `<div class="pv-answer">${esc(p.text)}</div>`;
  else if (p.text != null) html += `<pre class="pv-text">${esc(p.text)}${p.more ? "\n…" : ""}</pre>`;
  if (p.items?.length) {
    html += `<ul class="pv-items">${p.items.map(i => `<li>${i.dir ? GLYPHS.folder : GLYPHS.file}<span>${esc(i.name)}</span></li>`).join("")}${p.more ? `<li class="more">and more</li>` : ""}</ul>`;
  }
  html += `<dl class="pv-facts">${p.facts.map(f => `<dt>${esc(f.label)}</dt><dd>${esc(f.time != null ? when(f.time) : f.value)}</dd>`).join("")}</dl>`;
  return html;
}

// --- categories ----------------------------------------------------------------------------------
// Apps (and their windows), Files (and folders), Actions (commands and Settings pages), Clipboard:
// only that, more of it, and with nothing typed, all of it.
const CATS = [["apps", "Apps", GLYPHS.app], ["files", "Files", GLYPHS.file], ["actions", "Actions", GLYPHS.action], ["clipboard", "Clipboard", GLYPHS.clip]];

function drawCats() {
  const off = settings && !settings.clipboard;
  if (off && only === "clipboard") only = null;
  cats.innerHTML = CATS.filter(c => !(off && c[0] === "clipboard")).map(([id, name, g], i) =>
    `<button data-cat="${id}" aria-pressed="${only === id}" title="${name} (Ctrl+${i + 1})" aria-label="${name}">${g}<span>${name}</span></button>`).join("");
  input.placeholder = only ? `Search ${CATS.find(c => c[0] === only)[1]}` : "Fanlight Search";
}

function setOnly(id) {
  only = id && only !== id ? id : null;
  if (id === "clipboard" && settings && !settings.clipboard) only = null;
  drawCats();
  sel = 0;
  moved = false;
  run(input.value);
  input.focus();
}
cats.addEventListener("mousedown", e => e.preventDefault()); // keep the caret in the field
cats.addEventListener("click", e => {
  const b = e.target.closest("button[data-cat]");
  if (b) setOnly(b.dataset.cat);
});

/** Put an earlier search in the field, as a terminal's ↑ does, and search it. */
function recall(i) {
  back = i;
  input.value = i < 0 ? "" : past[i];
  input.setSelectionRange(input.value.length, input.value.length);
  run(input.value);
}

input.addEventListener("keydown", e => {
  const k = e.key;
  // ↑ steps back through earlier searches from an empty field, or from the top of the results of
  // one; Ctrl+↓ steps forward again. Otherwise the arrows move through the results.
  const stepping = sel === 0 && (!input.value || past.includes(input.value));
  if (k === "ArrowUp" && !e.shiftKey && (stepping || e.ctrlKey) && past.length) {
    e.preventDefault();
    const at = back >= 0 && past[back] === input.value ? back : past.indexOf(input.value);
    if (at + 1 < past.length) recall(at + 1);
  } else if (k === "ArrowDown" && e.ctrlKey) {
    e.preventDefault();
    const at = back >= 0 && past[back] === input.value ? back : past.indexOf(input.value);
    if (at >= 0) recall(at - 1);
  }
  else if (k === "ArrowDown") { e.preventDefault(); moved = true; step(true); }
  else if (k === "ArrowUp") { e.preventDefault(); moved = true; step(false); }
  else if (k === "Tab") { e.preventDefault(); moved = true; select(sel + (e.shiftKey ? -1 : 1)); }
  // ← and → move through the grid, unless they'd move the caret: → only from the end of what's
  // typed, ← once you've moved through the results or with nothing typed.
  else if ((k === "ArrowRight" || k === "ArrowLeft") && !e.shiftKey && !e.ctrlKey && inGrid()
    && input.selectionStart === input.value.length && input.selectionEnd === input.value.length
    && (k === "ArrowRight" || moved || !input.value)) {
    e.preventDefault();
    moved = true;
    select(sel + (k === "ArrowRight" ? 1 : -1));
  }
  else if (k === "PageDown") { e.preventDefault(); moved = true; select(Math.min(rows.length - 1, sel + 8)); }
  else if (k === "PageUp") { e.preventDefault(); moved = true; select(Math.max(0, sel - 8)); }
  // Space previews once you've moved through the results (while typing it's a space);
  // Shift+Space and Ctrl+Y do at any time.
  else if (k === " " && !e.ctrlKey && !e.altKey && (e.shiftKey || moved) && rows.length) { e.preventDefault(); setPreview(!previewing); }
  else if ((k === "y" || k === "Y") && e.ctrlKey && rows.length) { e.preventDefault(); setPreview(!previewing); }
  else if (e.ctrlKey && !e.shiftKey && !e.altKey && /^[1-4]$/.test(k)) {
    e.preventDefault();
    setOnly(CATS[Number(k) - 1][0]);
  } else if (k === "Backspace" && !input.value && only) {
    e.preventDefault();
    setOnly(null);
  }
  else if (k === "Enter") {
    e.preventDefault();
    choose(e.ctrlKey && e.shiftKey ? "admin" : e.ctrlKey ? (rows[sel]?.kind === "clip" ? "copy" : "reveal") : "open");
  } else if (k === "Escape") {
    e.preventDefault();
    // The preview first, then what's typed, then the category, then close, as Spotlight does.
    if (previewing) setPreview(false);
    else if (input.value) { input.value = ""; back = -1; moved = false; run(""); }
    else if (only) setOnly(null);
    else invoke("hide", { restore: true });
  } else if ((k === "c" || k === "C") && e.ctrlKey && input.selectionStart === input.selectionEnd && rows[sel]) {
    // Ctrl+C with nothing selected in the field copies the result.
    const h = rows[sel];
    if (["file", "folder", "url", "web", "calc", "clip"].includes(h.kind)) {
      e.preventDefault();
      choose("copy");
    }
  } else if (k === "Delete" && rows[sel]?.kind === "clip" && input.selectionStart === input.value.length) {
    e.preventDefault();
    invoke("forget", { key: rows[sel].key }).then(() => run(input.value));
  } else if (k === "," && e.ctrlKey) {
    e.preventDefault();
    showSettings();
  }
});

list.addEventListener("mousemove", e => {
  const el = e.target.closest("[data-i]");
  if (el && Number(el.dataset.i) !== sel) select(Number(el.dataset.i), false);
});
list.addEventListener("click", e => {
  const el = e.target.closest("[data-i]");
  if (!el) return;
  select(Number(el.dataset.i), false);
  choose(e.ctrlKey ? "reveal" : "open");
  input.focus();
});
list.addEventListener("mousedown", e => e.preventDefault()); // keep the caret in the field

// --- opening ------------------------------------------------------------------------------
listen("open", () => {
  // The window opens at its usual width (main.rs `place`), so without the preview.
  if (previewing) setPreview(false);
  moved = false;
  if (view !== "search") showSearch();
  back = -1;
  invoke("recent").then(r => { past = r; }).catch(() => {});
  // What was typed last stays, selected, so typing replaces it and Enter repeats it, as on a Mac.
  input.focus();
  input.select();
  if (input.value) run(input.value);
});
listen("view", v => { if (v === "settings") showSettings(); });
listen("settings", s => { settings = s; applyTheme(); drawCats(); });
window.addEventListener("focus", () => { if (view === "search") input.focus(); });

// --- size ---------------------------------------------------------------------------------
// The window is as tall as the panel, and wide with the preview: say so whenever that changes.
function fitNow() { invoke("fit", { height: panel.offsetHeight, wide: previewing }).catch(() => {}); }
new ResizeObserver(fitNow).observe(panel);

// --- settings ---------------------------------------------------------------------------------
const PALETTES = [
  ["oriel", "Default", "#3987e5"], ["catppuccin", "Catppuccin", "#cba6f7"], ["nord", "Nord", "#88c0d0"],
  ["gruvbox", "Gruvbox", "#8ec07c"], ["rose-pine", "Rosé Pine", "#c4a7e7"], ["solarized", "Solarized", "#2aa198"],
];
const label = spec => spec.split("+").map(p => p[0].toUpperCase() + p.slice(1)).join(" + ");

function showSearch() {
  view = "search";
  setv.hidden = true;
  list.hidden = false;
  input.disabled = false;
  input.focus();
  input.select();
  if (input.value) run(input.value);
}

async function showSettings() {
  if (previewing) setPreview(false);
  view = "settings";
  hello = await invoke("hello");
  settings = hello.settings;
  list.hidden = true;
  foot.hidden = true;
  setv.hidden = false;
  drawSettings();
  setv.querySelector("button, select, input")?.focus();
}

const sw = (id, on) => `<button class="switch" role="switch" data-set="${id}" aria-checked="${on}"></button>`;
const opt = (value, text, current) => `<option value="${esc(value)}"${value === current ? " selected" : ""}>${esc(text)}</option>`;
const setRow = (title, sub, control) => `<div class="set"><div class="label"><span>${title}</span>${sub ? `<small>${sub}</small>` : ""}</div>${control}</div>`;

function drawSettings(error = hello.hotkey_error) {
  const s = settings;
  setv.innerHTML = `
    <h2>Fanlight Settings <button class="btn primary" data-do="done">Done</button></h2>
    <h3>SHORTCUT</h3>
    ${setRow("Open Fanlight with", error ? `<span class="err">${esc(error)}</span>` : "Or click its icon in the notification area",
      `<select data-set="hotkey">${hello.hotkeys.map(h => opt(h, label(h), s.hotkey)).join("")}</select>`)}
    <h3>LOOK</h3>
    ${setRow("Theme", "", `<div class="seg">${["auto", "dark", "light"].map(t => `<button data-theme="${t}" aria-pressed="${s.theme === t}">${t[0].toUpperCase() + t.slice(1)}</button>`).join("")}</div>`)}
    ${setRow("Palette", PALETTES.find(p => p[0] === s.palette)?.[1] ?? "", `<div class="swatches">${PALETTES.map(([id, name, c]) => `<button class="swatch" data-palette="${id}" title="${name}" aria-label="${name}" aria-pressed="${s.palette === id}" style="background-color:${c}"></button>`).join("")}</div>`)}
    ${setRow("Glass", "See what’s behind the panel, blurred", sw("glass", s.glass))}
    <h3>SEARCH</h3>
    ${setRow("Files and folders", `${hello.files.toLocaleString()} in the index: Desktop, Documents, Downloads, Pictures, Music, Videos`, sw("files", s.files))}
    <div class="folders">${s.folders.map((f, i) => `<div class="folder"><span>${esc(f)}</span><button class="x" data-remove="${i}" title="Stop searching this folder">${GLYPHS.x}</button></div>`).join("")}</div>
    <div class="add"><input class="text-in" id="new-folder" placeholder="Another folder to search, like D:\\Work" spellcheck="false"><button class="btn" data-do="add-folder">Add</button></div>
    ${setRow("Open windows", "To switch to by their title", sw("windows", s.windows))}
    ${setRow("Apps", "How the Apps category (Ctrl+1) shows them", `<div class="seg">${[["grid", "Grid"], ["list", "List"]].map(([v, t]) => `<button data-apps="${v}" aria-pressed="${s.apps_view === v}">${t}</button>`).join("")}</div>`)}
    ${setRow("Search the web with", "For “Search the web”, and queries that start with ?", `<select data-set="engine">${hello.engines.map(([id, name]) => opt(id, name, s.engine)).join("")}</select>`)}
    <h3>CLIPBOARD</h3>
    ${setRow("Clipboard history", "Text you copy, to search and paste again. Type cb to see it all", sw("clipboard", s.clipboard))}
    ${setRow("Keep", "", `<select data-set="clipboard_size">${[25, 50, 100, 250, 500, 1000].map(n => opt(String(n), `The last ${n}`, String(s.clipboard_size))).join("")}</select>`)}
    ${setRow("Keep it after Fanlight quits", "Saved on this PC only. Off: in memory only", sw("keep_clipboard", s.keep_clipboard))}
    <h3>GENERAL</h3>
    ${setRow("Start at sign-in", "", `<button class="switch" role="switch" data-do="autostart" aria-checked="${hello.autostart}"></button>`)}
    ${setRow("What you’ve opened", "Fanlight ranks things you open often higher", `<button class="btn danger" data-do="clear-history">Forget</button>`)}
    ${setRow("File index", "Fanlight keeps it up to date by itself", `<button class="btn" data-do="reindex">Update now</button>`)}
    ${setRow("Quit Fanlight", "Until you start it again", `<button class="btn danger" data-do="quit">Quit</button>`)}
    <p class="about">Fanlight ${esc(hello.version)} · part of the Mullion suite, with Oriel and Lintel<br>Settings: ${esc(hello.settings_path)}</p>`;
  for (const [k, v] of Object.entries(s.clipboard ? {} : { clipboard_size: 1, keep_clipboard: 1 })) {
    void v;
    setv.querySelector(`[data-set="${k}"]`)?.closest(".set")?.style.setProperty("opacity", "0.45");
  }
}

async function save(change) {
  const next = { ...settings, ...change };
  try {
    settings = await invoke("save_settings", { settings: next });
    hello.hotkey_error = null;
    applyTheme();
    drawSettings(null);
  } catch (e) {
    drawSettings(String(e));
  }
}

setv.addEventListener("click", async e => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.dataset.set) return save({ [t.dataset.set]: t.getAttribute("aria-checked") !== "true" });
  if (t.dataset.theme) return save({ theme: t.dataset.theme });
  if (t.dataset.apps) return save({ apps_view: t.dataset.apps });
  if (t.dataset.palette) return save({ palette: t.dataset.palette });
  if (t.dataset.remove) return save({ folders: settings.folders.filter((_, i) => i !== Number(t.dataset.remove)) });
  switch (t.dataset.do) {
    case "done": return showSearch();
    case "add-folder": return addFolder();
    case "autostart":
      hello.autostart = await invoke("set_autostart", { enabled: !hello.autostart }).catch(() => hello.autostart);
      return drawSettings();
    case "clear-history":
      await invoke("clear_history");
      t.textContent = "Forgotten";
      return;
    case "reindex":
      await invoke("reindex");
      t.textContent = "Updating…";
      setTimeout(async () => { hello = await invoke("hello"); if (view === "settings") drawSettings(); }, 2500);
      return;
    case "quit": return invoke("quit");
  }
});
setv.addEventListener("change", e => {
  const t = e.target;
  if (t.dataset.set === "clipboard_size") return save({ clipboard_size: Number(t.value) });
  if (t.dataset.set) save({ [t.dataset.set]: t.value });
});
setv.addEventListener("keydown", e => {
  if (e.key === "Escape") { e.preventDefault(); showSearch(); }
  if (e.key === "Enter" && e.target.id === "new-folder") { e.preventDefault(); addFolder(); }
});
function addFolder() {
  const v = $("new-folder").value.trim().replace(/^"|"$/g, "");
  if (v) save({ folders: [...settings.folders, v] });
}

// --- start ----------------------------------------------------------------------------------
invoke("hello").then(h => {
  hello = h;
  settings = h.settings;
  applyTheme();
  drawCats();
});
drawCats();
}
