"use strict";
// Mullion's settings: which apps it runs (each one's switch starts or quits it), a way into each
// app's own window and settings, and Mullion's sign-in entry. While the window is open it asks
// every few seconds which apps are running, as they start and quit.
//
// The first time, it's a welcome instead: the same apps, each with its first step, switched on
// or off before any of them starts, and Start to start them.
//
// It says when there's a newer Mullion, and its Update button installs it (or opens the release
// page where it can't). The backend checks once a day and tells the page ("updates").
{
const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const listen = (event, fn) => T.event.listen(event, e => fn(e.payload));
const $ = id => document.getElementById(id);
const appsEl = $("apps"), general = $("general");

// A settings window, not a web page: no browser right-click menu, and none of its keys.
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
const svg = body => `<svg class="ic" viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;
const APP_GLYPH = svg(`<rect x="4" y="4" width="16" height="16" rx="4"/><path d="M4 9h16"/>`);
// On a button that opens a page in the browser, so it never does that unannounced.
const OUT = `<svg class="ic out" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 5h5v5M19 5l-8 8M17 14v4a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1h4"/></svg>`;
const web = (attrs, text) => `<button class="btn" ${attrs} title="Opens GitHub in your browser">${text}${OUT}</button>`;

let hello = null, apps = [], error = "";

// Windows' own light or dark, in the default palette; or the look for every app, while it's on.
const dark = matchMedia("(prefers-color-scheme: dark)");
const applyTheme = () => {
  const l = hello?.look?.on ? hello.look : { theme: "auto", palette: "oriel" };
  const root = document.documentElement;
  root.dataset.palette = l.palette;
  root.dataset.mode = l.theme === "auto" ? (dark.matches ? "dark" : "light") : l.theme;
};
dark.addEventListener("change", applyTheme);
applyTheme();

const PALETTES = [
  ["oriel", "Default", "#3987e5"], ["catppuccin", "Catppuccin", "#cba6f7"], ["nord", "Nord", "#88c0d0"],
  ["gruvbox", "Gruvbox", "#8ec07c"], ["rose-pine", "Rosé Pine", "#c4a7e7"], ["solarized", "Solarized", "#2aa198"],
];

const icons = {};
// The apps turned off in the welcome, until Start.
const off = new Set();

// What to do first with each app, for the welcome.
const FIRST = {
  Oriel: "Open it from Mullion’s icon when you want it",
  Lintel: "Shows up on your main screen straight away",
  Fanlight: "Press Alt+Space to try it",
  Bay: "Press Win+Ctrl+↑ to see every window",
  Sash: "Press Ctrl+Alt+← to snap a window to the left half",
  Mitre: "Open its settings to give each corner of your screens something to do",
};

const sw = (attrs, on, label) => `<button class="switch" role="switch" ${attrs} aria-checked="${on}" aria-label="${esc(label)}"></button>`;

function state(a) {
  if (!a.installed) return "Not installed";
  if (a.running) return a.on ? "Running" : "Running on its own";
  return a.on ? "Not running" : "Off";
}

const getIt = a => `<button class="btn" data-get="${esc(a.name)}">Get it</button>`;

// An app on its card: icon, name, what it is, a line under that, and its buttons.
const card = (a, line, side) => `<div class="app${a.installed ? "" : " missing"}">
  <span class="pic">${icons[a.name] ? `<img src="${icons[a.name]}" alt="">` : APP_GLYPH}</span>
  <div class="text">
    <span class="name">${esc(a.name)}</span>
    <span class="sub">${esc(a.about)}</span>
    ${line}
  </div>
  <div class="side">${side}</div>
</div>`;

const errLine = () => (error ? `<p class="err">${esc(error)}</p>` : "");

const autostartSet = () => `
  <div class="set"><div class="label"><span>Start Mullion when I sign in</span><small>And with it, every app that’s on</small></div>
    ${sw(`data-do="autostart"`, hello.autostart, "Start Mullion when I sign in")}</div>`;

// --- updates ---------------------------------------------------------------------------------
const clock = secs => new Date(secs * 1000).toLocaleString([], { weekday: "short", hour: "2-digit", minute: "2-digit" });

// What the check last found, under its switch.
function updateLine(u) {
  if (u.installing) return `Downloading Mullion ${esc(u.newer?.version)}… Mullion closes, updates and opens again`;
  if (u.checking) return "Checking…";
  if (u.error) return `<span class="bad">${esc(u.error)}</span>`;
  if (u.newer) return `Mullion ${esc(u.newer.version)} is out`;
  if (u.checked) return `Up to date · checked ${esc(clock(u.checked))}`;
  return "Once a day, from GitHub. Nothing else is sent";
}

const updatesSet = () => `
  <div class="set"><div class="label"><span>Check for updates</span><small>${updateLine(hello.updates)}</small></div>
    <div class="side">${hello.welcomed && !hello.updates.checking && !hello.updates.installing ? `<button class="btn" data-do="check">Check now</button>` : ""}
    ${sw(`data-do="updates"`, hello.check_updates, "Check for updates")}</div></div>`;

// The release notes' first line, which says what the release is, without Markdown's marks.
const gist = notes => (String(notes ?? "").split(/\r?\n/).find(l => l.trim()) ?? "").replace(/[*_`#>]/g, "").replace(/\[([^\]]*)\]\([^)]*\)/g, "$1").trim();

// Above the apps when there's a newer Mullion: what it is, and Update.
function drawUpdate() {
  const u = hello.updates, r = u.newer;
  $("update").hidden = !r;
  if (!r) return;
  $("update").innerHTML = `<div class="text">
      <span class="name">Mullion ${esc(r.version)} is out</span>
      <span class="sub">${esc(gist(r.notes)) || "A newer version of Mullion and its apps"}</span>
      ${u.can_install ? "" : `<span class="sub">This copy isn’t from Mullion’s installer, so Update opens the release page in your browser</span>`}
    </div>
    <div class="side">${web(`data-do="notes"`, "What’s new")}
      <button class="btn primary" data-do="update" ${u.installing ? "disabled" : ""}>${u.installing ? "Updating…" : "Update"}${u.can_install ? "" : OUT}</button></div>`;
}

function drawApps() {
  appsEl.innerHTML = apps.map(a => card(a,
    `<span class="state${a.running ? " up" : ""}">${state(a)}</span>`,
    !a.installed ? getIt(a)
      : `${a.shows && a.on ? `<button class="btn" data-settings="${esc(a.name)}">Settings</button>` : ""}
         ${!a.running && a.on ? `<button class="btn" data-start="${esc(a.name)}">Start</button>` : ""}
         ${sw(`data-app="${esc(a.name)}"`, a.on, `Run ${a.name}`)}`)).join("") + errLine();
}

function drawWelcome() {
  $("picks").innerHTML = apps.map(a => card(a,
    `<span class="hint">${a.installed ? esc(FIRST[a.name] ?? "") : "Not installed"}</span>`,
    a.installed ? sw(`data-pick="${esc(a.name)}"`, !off.has(a.name), `Run ${a.name}`) : getIt(a))).join("")
    // Nothing's wrong when the screen hitches as Lintel starts, but it's better said beforehand.
    + (apps.some(a => a.name === "Lintel" && a.installed && !off.has(a.name))
      ? `<p class="note">When Lintel starts, Windows makes room for its bar: the screen may pause for a moment and windows move down.</p>` : "")
    + errLine();
  $("first").innerHTML = autostartSet() + updatesSet();
}

const seg = (key, options, current) => `<div class="seg">${options.map(([v, t]) => `<button data-${key}="${v}" aria-pressed="${current === v}">${t}</button>`).join("")}</div>`;
const setRow = (title, sub, control) => `<div class="set"><div class="label"><span>${title}</span>${sub ? `<small>${sub}</small>` : ""}</div>${control}</div>`;

// One look for every app Mullion runs, or each app its own.
function drawLook() {
  const l = hello.look;
  $("look").innerHTML = setRow("Same look for every app", l.on ? "Each app’s own look comes back when this is off" : "Each app has its own, set in its settings",
      sw(`data-look="on"`, l.on, "Same look for every app"))
    + (l.on ? setRow("Theme", "", seg("theme", [["auto", "Auto"], ["dark", "Dark"], ["light", "Light"]], l.theme))
      + setRow("Palette", esc(PALETTES.find(p => p[0] === l.palette)?.[1] ?? ""), `<div class="swatches">${PALETTES.map(([id, name, c]) => `<button class="swatch" data-palette="${id}" title="${name}" aria-label="${name}" aria-pressed="${l.palette === id}" style="background-color:${c}"></button>`).join("")}</div>`)
      + setRow("Glass", "See what’s behind Lintel’s bar and Fanlight’s panel, blurred", sw(`data-look="glass"`, l.glass, "Glass")) : "");
}

$("look").addEventListener("click", async e => {
  const t = e.target.closest("button");
  if (!t) return;
  const change = t.dataset.look ? { [t.dataset.look]: t.getAttribute("aria-checked") !== "true" }
    : t.dataset.theme ? { theme: t.dataset.theme }
    : t.dataset.palette ? { palette: t.dataset.palette } : null;
  if (!change) return;
  hello.look = await invoke("set_look", { look: { ...hello.look, ...change } }).catch(() => hello.look);
  applyTheme();
  drawLook();
});

function drawGeneral() {
  general.innerHTML = `${autostartSet()}${updatesSet()}
    <div class="set"><div class="label"><span>Quit Mullion</span><small>And the apps it runs, until you start it again</small></div>
      <button class="btn danger" data-do="quit">Quit</button></div>`;
  $("about").innerHTML = `Settings: ${esc(hello.settings_path)}`;
}

// Mullion's version and each installed app's, which should be the same: an app that isn't was
// put there some other way (its own installer, or a portable copy).
function drawAbout() {
  $("versions").innerHTML = setRow(`Mullion ${esc(hello.version)}`, "What’s new and Source open GitHub in your browser",
      `<div class="side">${web(`data-page="notes"`, "What’s new")}${web(`data-page="source"`, "Source")}</div>`)
    + apps.filter(a => a.installed).map(a => setRow(esc(a.name),
      a.version && a.version !== hello.version ? "Not the same as Mullion’s: installing Mullion again brings it in step" : "",
      `<span class="ver">${esc(a.version ?? "Unknown")}</span>`)).join("");
}

$("versions").addEventListener("click", e => {
  const t = e.target.closest("button");
  if (t?.dataset.page) invoke("open_page", { page: t.dataset.page });
});

async function refresh() {
  try {
    apps = await invoke("apps");
    drawApps();
  } catch (e) {
    console.warn(e);
  }
}

// The welcome or the settings, whichever it's at.
function draw() {
  const welcome = !hello.welcomed;
  $("welcome").hidden = !welcome;
  $("settings").hidden = welcome;
  $("title").textContent = welcome ? "Welcome to Mullion" : "Mullion";
  if (welcome) drawWelcome();
  else { drawUpdate(); drawApps(); drawLook(); drawGeneral(); drawAbout(); }
}

$("picks").addEventListener("click", e => {
  const t = e.target.closest("button");
  if (t?.dataset.pick) off.has(t.dataset.pick) ? off.delete(t.dataset.pick) : off.add(t.dataset.pick);
  else if (t?.dataset.get) invoke("get_app");
  drawWelcome();
});

$("go").addEventListener("click", async () => {
  error = "";
  try {
    await invoke("welcome", { off: [...off] });
    hello.welcomed = true;
    apps = await invoke("apps");
  } catch (err) {
    error = String(err);
  }
  draw();
  // The apps take a moment to start.
  setTimeout(refresh, 800);
});

appsEl.addEventListener("click", async e => {
  const t = e.target.closest("button");
  if (!t) return;
  error = "";
  try {
    if (t.dataset.app) apps = await invoke("set_app", { name: t.dataset.app, on: t.getAttribute("aria-checked") !== "true" });
    else if (t.dataset.start) await invoke("set_app", { name: t.dataset.start, on: true });
    else if (t.dataset.settings) await invoke("open_app", { name: t.dataset.settings, settings: true });
    else if (t.dataset.get) await invoke("get_app");
  } catch (err) {
    error = String(err);
  }
  drawApps();
  // The app takes a moment to start or quit.
  setTimeout(refresh, 800);
});

const onGeneral = async e => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.dataset.do === "autostart") {
    hello.autostart = await invoke("set_autostart", { enabled: t.getAttribute("aria-checked") !== "true" }).catch(() => hello.autostart);
    draw();
  } else if (t.dataset.do === "updates") {
    hello.check_updates = await invoke("set_check_updates", { on: t.getAttribute("aria-checked") !== "true" }).catch(() => hello.check_updates);
    draw();
  } else if (t.dataset.do === "check") {
    invoke("check_updates");
  } else if (t.dataset.do === "quit") {
    invoke("quit");
  }
};
general.addEventListener("click", onGeneral);
$("first").addEventListener("click", onGeneral);

listen("autostart", on => { if (hello) { hello.autostart = on; draw(); } });
listen("updates", u => { if (hello) { hello.updates = u; draw(); } });

$("update").addEventListener("click", e => {
  const t = e.target.closest("button");
  if (t?.dataset.do === "update") invoke("update").catch(err => { error = String(err); draw(); });
  else if (t?.dataset.do === "notes") invoke("release_notes");
});

async function start() {
  hello = await invoke("hello");
  apps = hello.apps;
  for (const a of apps) if (a.icon) icons[a.name] = a.icon;
  for (const a of apps) if (!a.on) off.add(a.name);
  if (hello.icon) $("logo").innerHTML = `<img src="${hello.icon}" alt="">`;
  applyTheme();
  draw();
  invoke("ready");
  setInterval(() => { if (!document.hidden && hello.welcomed) refresh(); }, 3000);
}
start();
}
