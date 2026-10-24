// Loaded into the bar and menu pages by serve.py, before ui/common.js: stands in for Tauri and
// passes everything to the harness page around them (host.js).
(() => {
  const host = window.parent.LintelHost;
  const label = location.pathname.endsWith("menu.html") ? "menu" : "bar";

  // A fixed clock for screenshots: ?time=2026-10-24T17:42 on the harness page.
  const time = new URLSearchParams(window.parent.location.search).get("time");
  if (time) {
    const RealDate = Date;
    const offset = new RealDate(time).getTime() - RealDate.now();
    if (Number.isFinite(offset)) {
      globalThis.Date = class extends RealDate {
        constructor(...a) { super(...(a.length ? a : [RealDate.now() + offset])); }
        static now() { return RealDate.now() + offset; }
      };
    }
  }
  window.__TAURI__ = {
    core: { invoke: (cmd, args = {}) => host.invoke(label, cmd, args) },
    webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (event, fn) => Promise.resolve(host.listen(label, event, payload => fn({ event, payload }))) }) },
  };
})();
