"use strict";
// Shared by the bar (index.html) and its menus (menu.html): talking to the backend, the theme,
// number formats and the icons.
const Lintel = (() => {
  const T = window.__TAURI__;
  // This window's own events, plus those sent to every window (settings).
  const me = T.webviewWindow.getCurrentWebviewWindow();
  const invoke = (cmd, args) => T.core.invoke(cmd, args);
  const listen = (event, fn) => me.listen(event, e => fn(e.payload));

  // A bar, not a web page: no browser right-click menu (Back, Reload, Inspect), and none of its
  // keys (reload, find, print, zoom, developer tools).
  document.addEventListener("contextmenu", e => e.preventDefault());
  document.addEventListener("keydown", e => {
    const k = e.key.toLowerCase();
    if (["f5", "f7", "f12", "browserback", "browserforward"].includes(k)
      || (e.ctrlKey && ["r", "f", "g", "p", "u", "j", "+", "-", "=", "0"].includes(k))
      || (e.ctrlKey && e.shiftKey && ["i", "c"].includes(k))
      || (e.altKey && ["arrowleft", "arrowright"].includes(k))) e.preventDefault();
  });
  document.addEventListener("wheel", e => { if (e.ctrlKey) e.preventDefault(); }, { passive: false });

  const dark = matchMedia("(prefers-color-scheme: dark)");
  let theme = "auto";
  function applyTheme(settings) {
    theme = settings.theme;
    const root = document.documentElement;
    root.dataset.palette = settings.palette;
    root.dataset.glass = settings.glass ? "on" : "off";
    root.dataset.mode = theme === "auto" ? (dark.matches ? "dark" : "light") : theme;
  }
  dark.addEventListener("change", () => {
    if (theme === "auto") document.documentElement.dataset.mode = dark.matches ? "dark" : "light";
  });

  const esc = s => String(s ?? "").replace(/[&<>"']/g, c => `&#${c.charCodeAt(0)};`);

  /** 1234567 -> "1.2 MB" */
  function bytes(n, digits = 1) {
    const units = ["B", "KB", "MB", "GB", "TB"];
    let i = 0;
    while (n >= 1024 && i < units.length - 1) { n /= 1024; i++; }
    return `${n.toFixed(i && n < 100 ? digits : 0)} ${units[i]}`;
  }
  const rate = n => `${bytes(n)}/s`;

  /** 83 -> "1:23", 3725 -> "1:02:05" */
  function clock(secs) {
    const s = Math.max(0, Math.floor(secs));
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), ss = String(s % 60).padStart(2, "0");
    return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
  }

  /** 5400 -> "1 h 30 min" */
  function duration(secs) {
    const m = Math.round(secs / 60), h = Math.floor(m / 60);
    return h ? `${h} h ${m % 60} min` : `${m} min`;
  }

  // Players as Windows' media controls name them, as people know them.
  const PLAYERS = {
    spotify: "Spotify", spotifymusic: "Spotify", chrome: "Chrome", msedge: "Edge", firefox: "Firefox",
    zunemusic: "Media Player", zunevideo: "Films & TV", vlc: "VLC", applemusicwin: "Apple Music", itunes: "iTunes",
    tidal: "TIDAL", deezer: "Deezer", musicbee: "MusicBee", foobar2000: "foobar2000", brave: "Brave", opera: "Opera",
  };
  /** "Spotify.exe", "Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic" or "Helium.LYMXGRRS252…" to a name (from Oriel). */
  function player(id = "") {
    let s = id.replace(/\.exe$/i, "");
    if (/^[0-9A-F]{16}$/.test(s)) return "Firefox"; // Firefox's id is a hash of the folder it's installed in
    if (s.includes("!")) s = s.split("!")[0].split("_")[0]; // Store app: Publisher.Name_hash!App
    const parts = s.split(".").filter(p => p && !/^[0-9A-Z]{12,}$/.test(p) && !p.startsWith("_crx_"));
    for (const p of [s, ...parts]) {
      const known = PLAYERS[p.toLowerCase()];
      if (known) return known;
    }
    return parts.at(-1) ?? "";
  }

  // Programs in the volume mixer, by file name.
  const PROGRAMS = {
    chrome: "Chrome", msedge: "Edge", firefox: "Firefox", brave: "Brave", opera: "Opera", spotify: "Spotify",
    discord: "Discord", steam: "Steam", teams: "Teams", "ms-teams": "Teams", slack: "Slack", zoom: "Zoom", vlc: "VLC",
    obs64: "OBS", code: "VS Code", explorer: "File Explorer",
  };
  const program = exe => PROGRAMS[(exe || "").toLowerCase()] || exe || "App";

  // Icons: 24-unit boxes, drawn with currentColor so they follow the theme.
  const svg = (body, cls = "") => `<svg class="ic ${cls}" viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;
  const icons = {
    logo: () => svg(`<rect x="3" y="3" width="18" height="18" rx="5" class="fill"/><rect x="7" y="7.2" width="10" height="2.6" rx="1.3" class="cut"/>`, "logo"),
    play: () => svg(`<path d="M8 5.5v13l10.5-6.5z" class="fill"/>`),
    pause: () => svg(`<rect x="6.5" y="5.5" width="3.8" height="13" rx="1" class="fill"/><rect x="13.7" y="5.5" width="3.8" height="13" rx="1" class="fill"/>`),
    prev: () => svg(`<path d="M18 6v12l-8.5-6z" class="fill"/><rect x="5.5" y="6" width="2.6" height="12" rx="1" class="fill"/>`),
    next: () => svg(`<path d="M6 6v12l8.5-6z" class="fill"/><rect x="15.9" y="6" width="2.6" height="12" rx="1" class="fill"/>`),
    note: () => svg(`<path d="M9 18V6l10-2v12"/><circle cx="6.5" cy="18" r="2.5" class="fill"/><circle cx="16.5" cy="16" r="2.5" class="fill"/>`),
    speaker(volume, muted) {
      const waves = muted ? `<path d="M16 9.5l5 5M21 9.5l-5 5"/>`
        : (volume > 0 ? `<path d="M15.5 9.2a4 4 0 0 1 0 5.6"/>` : "") + (volume > 50 ? `<path d="M18.3 6.6a7.6 7.6 0 0 1 0 10.8"/>` : "");
      return svg(`<path d="M4 9.5h3l4.5-4v13l-4.5-4H4z" class="fill"/>${waves}`);
    },
    wifi(signal) {
      // Four steps, as Windows shows it; the arcs the signal doesn't reach are dimmed.
      const on = signal >= 75 ? 4 : signal >= 50 ? 3 : signal >= 25 ? 2 : 1;
      const arc = (i, d) => `<path d="${d}" class="${i < on ? "" : "dim"}"/>`;
      return svg(arc(3, "M2.5 9a14 14 0 0 1 19 0") + arc(2, "M5.5 12.2a9.5 9.5 0 0 1 13 0") + arc(1, "M8.5 15.3a5 5 0 0 1 7 0")
        + `<circle cx="12" cy="18.6" r="1.6" class="fill"/>`);
    },
    wired: () => svg(`<rect x="3.5" y="4.5" width="17" height="11" rx="2"/><path d="M8 19.5h8M12 15.5v4"/>`),
    offline: () => svg(`<path d="M2.5 9a14 14 0 0 1 19 0M5.5 12.2a9.5 9.5 0 0 1 13 0M8.5 15.3a5 5 0 0 1 7 0" class="dim"/><path d="M4 4l16 16"/>`),
    battery(pct, plugged) {
      const w = Math.max(1.5, (Math.min(100, pct) / 100) * 13);
      const bolt = plugged ? `<path d="M11.8 8.2l-2.6 4.3h3l-1.9 3.6" class="bolt"/>` : "";
      return svg(`<rect x="2.5" y="7" width="17" height="10" rx="2.6"/><path d="M21.5 10.5v3" /><rect x="4.5" y="9" width="${w}" height="6" rx="1.2" class="fill${pct <= 20 && !plugged ? " low" : ""}"/>${bolt}`, "battery");
    },
    check: () => svg(`<path d="M5 12.5l4.5 4.5L19 7.5"/>`),
    chevron: dir => svg(dir === "left" ? `<path d="M14.5 6l-6 6 6 6"/>` : `<path d="M9.5 6l6 6-6 6"/>`),
  };

  return { invoke, listen, applyTheme, esc, bytes, rate, clock, duration, player, program, icons };
})();
