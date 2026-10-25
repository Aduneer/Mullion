"""Renders the frames of the README's animation: the preview with ?demo=1 in headless Chromium,
recorded as it plays (the DevTools protocol's screencast), then evened out to a steady frame rate.

One recording of one page load keeps every frame in step with the last (a fresh load per frame
drifts by however long that load took). Standard library only, so the WebSocket the protocol
runs over is done here in a few lines.

    python -I tools/preview/demo.py <browser.exe> <url> <out dir> [start ms] [end ms] [fps] [width] [height]
"""
import base64, json, os, select, socket, struct, subprocess, sys, tempfile, time, urllib.request



class DevTools:
    """Just enough of a WebSocket client for the DevTools protocol: text frames, one at a time."""

    def __init__(self, ws_url):
        host, path = ws_url.removeprefix("ws://").split("/", 1)
        name, port = host.split(":")
        self.sock = socket.create_connection((name, int(port)))
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall((f"GET /{path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                           f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        head = b""
        while b"\r\n\r\n" not in head:
            head += self.sock.recv(1)
        if b" 101 " not in head.split(b"\r\n")[0]:
            raise RuntimeError(head.decode(errors="replace"))
        self.next_id = 0
        self.events, self.replies = [], {}

    def _read(self, n):
        data = b""
        while len(data) < n:
            chunk = self.sock.recv(n - len(data))
            if not chunk:
                raise ConnectionError("the browser closed the connection")
            data += chunk
        return data

    def _receive(self):
        message = b""
        while True:
            b0, b1 = self._read(2)
            n = b1 & 0x7F
            if n == 126:
                n = struct.unpack(">H", self._read(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self._read(8))[0]
            message += self._read(n)
            if b0 & 0x80:  # the last fragment
                return json.loads(message)

    def post(self, method, **params):
        """Send a command without waiting for its reply; returns its id."""
        self.next_id += 1
        body = json.dumps({"id": self.next_id, "method": method, "params": params}).encode()
        mask = os.urandom(4)
        n = len(body)
        head = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n))
        self.sock.sendall(head + mask + bytes(c ^ mask[i % 4] for i, c in enumerate(body)))
        return self.next_id

    def _next(self, timeout):
        """File the next message as a reply or an event; False if none came within the timeout."""
        if timeout is not None and not select.select([self.sock], [], [], timeout)[0]:
            return False
        m = self._receive()
        if "id" in m:
            self.replies[m["id"]] = m
        else:
            self.events.append(m)
        return True

    def reply(self, id, timeout=None):
        """The reply to command `id`, or None if it hasn't come within the timeout."""
        while id not in self.replies:
            if not self._next(timeout):
                return None
        m = self.replies.pop(id)
        if "error" in m:
            raise RuntimeError(m["error"])
        return m.get("result", {})

    def send(self, method, **params):
        return self.reply(self.post(method, **params))

    def wait_for(self, event, timeout=None):
        """The next `event`, or None if nothing came within the timeout."""
        while True:
            for i, m in enumerate(self.events):
                if m.get("method") == event:
                    return self.events.pop(i)
            if not self._next(timeout):
                return None


def main():
    browser, url, out = sys.argv[1:4]
    start, end, fps, width, height = (float(a) for a in (sys.argv[4:9] + ["300", "15800", "20", "1280", "800"][len(sys.argv[4:9]):]))
    width, height = int(width), int(height)
    os.makedirs(out, exist_ok=True)
    profile = tempfile.mkdtemp(prefix="fanlight-demo-profile-")
    proc = subprocess.Popen([browser, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--no-first-run",
                             f"--user-data-dir={profile}", "--remote-debugging-port=0", "--remote-allow-origins=*",
                             f"--window-size={width},{height}", "about:blank"],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        # It picks a free port and writes it into the profile.
        for _ in range(100):
            try:
                port = open(os.path.join(profile, "DevToolsActivePort")).read().split()[0]
                pages = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=2))
                page = next(p for p in pages if p["type"] == "page")
                break
            except (OSError, IndexError, StopIteration):
                time.sleep(0.1)
        else:
            raise RuntimeError("the browser didn't start")
        dt = DevTools(page["webSocketDebuggerUrl"])
        dt.send("Page.enable")
        dt.send("Emulation.setDefaultBackgroundColorOverride", color={"r": 0, "g": 0, "b": 0, "a": 0})
        # The window size leaves the page a little less than asked for; the page gets just that.
        dt.send("Emulation.setDeviceMetricsOverride", width=width, height=height, deviceScaleFactor=1, mobile=False)
        dt.send("Page.startScreencast", format="png", maxWidth=width, maxHeight=height)
        dt.send("Page.navigate", url=url)
        # Every frame the page draws, with when (in ms from the first), until the tour is over.
        # A page that has stopped changing draws no more frames, so the clock ends it too.
        shots, first, began = [], None, None
        while True:
            m = dt.wait_for("Page.screencastFrame", 0.5)
            if m is None:
                if began is not None and time.monotonic() - began > end / 1000:
                    break
                continue
            m = m["params"]
            began = began or time.monotonic()
            dt.post("Page.screencastFrameAck", sessionId=m["sessionId"])
            t = m["metadata"]["timestamp"] * 1000
            first = first if first is not None else t
            shots.append((t - first, m["data"]))
            if t - first > end:
                break
        dt.send("Page.stopScreencast")
        # Then an even frame rate: at each tick, the last frame drawn by then.
        step, k, i = 1000 / fps, 0, 0
        while start + k * step <= end:
            while i + 1 < len(shots) and shots[i + 1][0] <= start + k * step:
                i += 1
            with open(os.path.join(out, f"{k:03d}.png"), "wb") as f:
                f.write(base64.b64decode(shots[i][1]))
            k += 1
        print(f"{k} frames from {len(shots)} drawn")
    finally:
        proc.terminate()


if __name__ == "__main__":
    main()
