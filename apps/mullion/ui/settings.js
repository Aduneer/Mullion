"use strict";
// Mullion's settings: which apps it runs (each one's switch starts or quits it), a way into each
// app's own window and settings, and Mullion's sign-in entry. While the window is open it asks
// every few seconds which apps are running, as they start and quit.
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

// Windows' own light or dark, in the default palette.
const dark = matchMedia("(prefers-color-scheme: dark)");
const applyTheme = () => { document.documentElement.dataset.mode = dark.matches ? "dark" : "light"; };
dark.addEventListener("change", applyTheme);
applyTheme();

let hello = null, apps = [], error = "";
const icons = {};

const sw = (attrs, on, label) => `<button class="switch" role="switch" ${attrs} aria-checked="${on}" aria-label="${esc(label)}"></button>`;

function state(a) {
  if (!a.installed) return "Not installed";
  if (a.running) return a.on ? "Running" : "Running on its own";
  return a.on ? "Not running" : "Off";
}

function drawApps() {
  appsEl.innerHTML = apps.map(a => {
    const pic = icons[a.name] ? `<img src="${icons[a.name]}" alt="">` : APP_GLYPH;
    const side = !a.installed
      ? `<button class="btn" data-get="${esc(a.name)}">Get it</button>`
      : `${a.shows && a.on ? `<button class="btn" data-settings="${esc(a.name)}">Settings</button>` : ""}
         ${!a.running && a.on ? `<button class="btn" data-start="${esc(a.name)}">Start</button>` : ""}
         ${sw(`data-app="${esc(a.name)}"`, a.on, `Run ${a.name}`)}`;
    return `<div class="app${a.installed ? "" : " missing"}">
      <span class="pic">${pic}</span>
      <div class="text">
        <span class="name">${esc(a.name)}</span>
        <span class="sub">${esc(a.about)}</span>
        <span class="state${a.running ? " up" : ""}">${state(a)}</span>
      </div>
      <div class="side">${side}</div>
    </div>`;
  }).join("") + (error ? `<p class="err">${esc(error)}</p>` : "");
}

function drawGeneral() {
  general.innerHTML = `
    <div class="set"><div class="label"><span>Start Mullion when I sign in</span><small>And with it, every app that’s on</small></div>
      ${sw(`data-do="autostart"`, hello.autostart, "Start Mullion when I sign in")}</div>
    <div class="set"><div class="label"><span>Quit Mullion</span><small>And the apps it runs, until you start it again</small></div>
      <button class="btn danger" data-do="quit">Quit</button></div>`;
  $("about").innerHTML = `Mullion ${esc(hello.version)} · each app is its own program, and works without the others<br>Settings: ${esc(hello.settings_path)}`;
}

async function refresh() {
  try {
    apps = await invoke("apps");
    drawApps();
  } catch (e) {
    console.warn(e);
  }
}

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

general.addEventListener("click", async e => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.dataset.do === "autostart") {
    hello.autostart = await invoke("set_autostart", { enabled: t.getAttribute("aria-checked") !== "true" }).catch(() => hello.autostart);
    drawGeneral();
  } else if (t.dataset.do === "quit") {
    invoke("quit");
  }
});

listen("autostart", on => { if (hello) { hello.autostart = on; drawGeneral(); } });

async function start() {
  hello = await invoke("hello");
  apps = hello.apps;
  for (const a of apps) if (a.icon) icons[a.name] = a.icon;
  if (hello.icon) $("logo").innerHTML = `<img src="${hello.icon}" alt="">`;
  drawApps();
  drawGeneral();
  invoke("ready");
  setInterval(() => { if (!document.hidden) refresh(); }, 3000);
}
start();
}
