// Loaded into the panel page by serve.py, before ui/panel.js: stands in for Tauri and passes
// everything to the harness page around it (host.js).
(() => {
  const host = window.parent.FanlightHost;
  window.__TAURI__ = {
    core: { invoke: (cmd, args = {}) => host.invoke(cmd, args) },
    webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (event, fn) => Promise.resolve(host.listen(event, payload => fn({ event, payload }))) }) },
  };
})();
