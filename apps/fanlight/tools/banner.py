# Draws the README banners, assets/banner-dark.svg and banner-light.svg, from the logo (assets/fanlight.svg).
#   python -I tools/banner.py assets
import re, sys
out = sys.argv[1]
logo = open(f"{out}/fanlight.svg", encoding="utf-8").read()
inner = re.search(r"<svg[^>]*>(.*)</svg>", logo, re.S).group(1)
inner = re.sub(r"<title>.*?</title>", "", inner)

def banner(mode):
    dark = mode == "dark"
    ink, muted, field, edge, hi = (("#f0efec", "#898781", "#1a1a19", "rgba(255,255,255,0.10)", "#262624") if dark
                                   else ("#0b0b0b", "#6b6a66", "#ffffff", "rgba(11,11,11,0.12)", "#ecebe6"))
    font = 'font-family="Segoe UI Variable Display, Segoe UI, system-ui, -apple-system, sans-serif"'
    small = 'font-family="Segoe UI Variable Text, Segoe UI, system-ui, -apple-system, sans-serif"'
    # The search field, with something typed and the shortcut that opens it.
    fx, fy, fw, fh = 520, 362, 660, 54
    cy = fy + fh / 2
    items = f'''
    <rect x="{fx}" y="{fy}" width="{fw}" height="{fh}" rx="12" fill="{field}" stroke="{edge}"/>
    <g transform="translate({fx+18},{cy-11}) scale(0.92)" fill="none" stroke="{muted}" stroke-width="2.2" stroke-linecap="round">
      <circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5l5 5"/>
    </g>
    <text x="{fx+56}" y="{cy+8}" font-size="22" font-weight="300" fill="{ink}" {font}>lisbon trip</text>
    <rect x="{fx+147}" y="{cy-13}" width="2" height="26" fill="#3987e5"/>
    <rect x="{fx+fw-128}" y="{cy-13}" width="110" height="26" rx="6" fill="{hi}" stroke="{edge}"/>
    <text x="{fx+fw-73}" y="{cy+5}" font-size="13" fill="{muted}" text-anchor="middle" {small}>Alt + Space</text>'''
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 560" width="1280" height="560">
<title>Fanlight</title>
<defs><radialGradient id="glow"><stop offset="0" stop-color="#6b86ea" stop-opacity="{0.38 if dark else 0.28}"/><stop offset="1" stop-color="#6b86ea" stop-opacity="0"/></radialGradient></defs>
<ellipse cx="300" cy="280" rx="300" ry="250" fill="url(#glow)"/>
<g transform="translate(130,110) scale(0.332)">{inner}</g>
<text x="516" y="262" font-size="132" font-weight="700" letter-spacing="-3" fill="{ink}" {font}>Fanlight</text>
<text x="522" y="322" font-size="34" fill="{muted}" {font}>Search and launch for Windows</text>
{items}
</svg>
'''

for m in ("dark", "light"):
    open(f"{out}/banner-{m}.svg", "w", encoding="utf-8").write(banner(m))
