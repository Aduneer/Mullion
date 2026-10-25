# The app icons in src-tauri/icons, from the logo rendered at 1024 px (tools/preview/shots.ps1 -Icons does both). Needs Pillow.
import sys
from PIL import Image
src, out = sys.argv[1], sys.argv[2]
big = Image.open(src).convert("RGBA")
size = lambda n: big.resize((n, n), Image.LANCZOS)
size(32).save(f"{out}/32x32.png")
size(128).save(f"{out}/128x128.png")
size(256).save(f"{out}/128x128@2x.png")
size(512).save(f"{out}/icon.png")
size(256).save(f"{out}/icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
