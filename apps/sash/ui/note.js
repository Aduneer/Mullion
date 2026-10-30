"use strict";
// Sash's note: says why the window in front didn't move. Sash places and shows its window once
// the page has its size (`note_ready`), and closes it after a few seconds.
{
const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const note = document.getElementById("note");
const text = document.getElementById("text");

document.addEventListener("contextmenu", e => e.preventDefault());

function show(words) {
  text.textContent = words;
  const r = note.getBoundingClientRect();
  invoke("note_ready", { w: Math.ceil(r.width), h: Math.ceil(r.height) });
}

invoke("note_hello").then(h => {
  const root = document.documentElement;
  document.getElementById("logo").src = h.icon;
  root.dataset.palette = h.palette;
  root.dataset.mode = h.theme === "auto" ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light") : h.theme;
  show(h.text);
});
// A second note while this one's up says the new thing instead.
T.event.listen("note", e => show(e.payload));
}
