# Serves the checkout for the preview harness (tools/preview/index.html): the real ui/ page, with
# the mock backend (mock.js) loaded before its script. No Rust build needed.
#   python -I tools/preview/serve.py [port]      then open http://127.0.0.1:5297/tools/preview/
import http.server, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 5297

class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        path = self.path.split("?")[0]
        if path == "/ui/index.html":
            html = open(os.path.join(ROOT, "ui", "index.html"), encoding="utf-8").read()
            html = html.replace('<script src="panel.js"></script>', '<script src="/tools/preview/mock.js"></script><script src="panel.js"></script>', 1)
            body = html.encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        super().do_GET()
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()
    def log_message(self, *a):
        pass

print(f"serving {ROOT} on http://127.0.0.1:{PORT}/tools/preview/", flush=True)
http.server.ThreadingHTTPServer(("127.0.0.1", PORT), lambda *a: Handler(*a, directory=ROOT)).serve_forever()
