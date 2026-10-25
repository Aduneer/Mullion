# Serves the settings page for the preview: the real ui/ page, with the mock backend (mock.js)
# loaded before its script, and the apps' icons from their own folders. No Rust build needed.
#   python -I tools/preview/serve.py [port]      then open http://127.0.0.1:5295/ui/index.html
import http.server, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
APPS = os.path.dirname(ROOT)
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 5295

class Handler(http.server.SimpleHTTPRequestHandler):
    def send_bytes(self, body, kind):
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = self.path.split("?")[0]
        if path == "/ui/index.html":
            html = open(os.path.join(ROOT, "ui", "index.html"), encoding="utf-8").read()
            html = html.replace('<script src="settings.js"></script>', '<script src="/tools/preview/mock.js"></script><script src="settings.js"></script>', 1)
            return self.send_bytes(html.encode("utf-8"), "text/html; charset=utf-8")
        # /icons/fanlight.png: that app's icon, as its exe has it.
        if path.startswith("/icons/") and path.endswith(".png"):
            app = os.path.basename(path)[:-4]
            if app.isalpha():
                icon = os.path.join(APPS, app, "src-tauri", "icons", "128x128.png")
                if os.path.isfile(icon):
                    return self.send_bytes(open(icon, "rb").read(), "image/png")
            return self.send_error(404)
        super().do_GET()
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()
    def log_message(self, *a):
        pass

print(f"serving {ROOT} on http://127.0.0.1:{PORT}/ui/index.html", flush=True)
http.server.ThreadingHTTPServer(("127.0.0.1", PORT), lambda *a: Handler(*a, directory=ROOT)).serve_forever()
