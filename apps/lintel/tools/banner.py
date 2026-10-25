# Draws the README banners, assets/banner-dark.svg and banner-light.svg, from the logo (assets/lintel.svg).
#   python -I tools/banner.py assets
import re, sys
out = sys.argv[1]
logo = open(f"{out}/lintel.svg", encoding="utf-8").read()
inner = re.search(r"<svg[^>]*>(.*)</svg>", logo, re.S).group(1)
inner = re.sub(r"<title>.*?</title>", "", inner)

def banner(mode):
    dark = mode == "dark"
    ink, muted, bar, edge, hi = (("#f0efec", "#898781", "#1a1a19", "rgba(255,255,255,0.10)", "#262624") if dark
                                 else ("#0b0b0b", "#6b6a66", "#ffffff", "rgba(11,11,11,0.12)", "#ecebe6"))
    font = 'font-family="Segoe UI Variable Display, Segoe UI, system-ui, -apple-system, sans-serif"'
    small = 'font-family="Segoe UI Variable Text, Segoe UI, system-ui, -apple-system, sans-serif"'
    # The bar: 640 x 40 at (520, 360).
    bx, by, bw, bh = 520, 368, 660, 42
    cy = by + bh / 2
    items = f'''
    <rect x="{bx}" y="{by}" width="{bw}" height="{bh}" rx="11" fill="{bar}" stroke="{edge}"/>
    <rect x="{bx+16}" y="{cy-8}" width="16" height="16" rx="4.5" fill="{ink}"/>
    <rect x="{bx+19.5}" y="{cy-4.5}" width="9" height="2.6" rx="1.3" fill="{bar}"/>
    <text x="{bx+46}" y="{cy+5}" font-size="15" font-weight="700" fill="{ink}" {small}>Lintel</text>
    <text x="{bx+bw-370}" y="{cy+5}" font-size="10.5" font-weight="600" letter-spacing="0.5" fill="{muted}" {small}>CPU</text>
    <text x="{bx+bw-345}" y="{cy+5}" font-size="14" fill="{ink}" {small}>12%</text>
    <text x="{bx+bw-305}" y="{cy+5}" font-size="10.5" font-weight="600" letter-spacing="0.5" fill="{muted}" {small}>MEM</text>
    <text x="{bx+bw-276}" y="{cy+5}" font-size="14" fill="{ink}" {small}>41%</text>
    <g transform="translate({bx+bw-230},{cy-8}) scale(0.667)" fill="none" stroke="{ink}" stroke-width="2" stroke-linecap="round">
      <path d="M4 9.5h3l4.5-4v13l-4.5-4H4z" fill="{ink}" stroke="none"/><path d="M15.5 9.2a4 4 0 0 1 0 5.6"/><path d="M18.3 6.6a7.6 7.6 0 0 1 0 10.8"/>
    </g>
    <g transform="translate({bx+bw-196},{cy-8}) scale(0.667)" fill="none" stroke="{ink}" stroke-width="2" stroke-linecap="round">
      <path d="M2.5 9a14 14 0 0 1 19 0M5.5 12.2a9.5 9.5 0 0 1 13 0M8.5 15.3a5 5 0 0 1 7 0"/><circle cx="12" cy="18.6" r="1.6" fill="{ink}" stroke="none"/>
    </g>
    <rect x="{bx+bw-158}" y="{cy-12}" width="146" height="24" rx="6" fill="{hi}"/>
    <text x="{bx+bw-150}" y="{cy+5}" font-size="14" fill="{muted}" {small}>Sat 24 Oct</text>
    <text x="{bx+bw-57}" y="{cy+5}" font-size="14" font-weight="600" fill="{ink}" {small}>17:42</text>'''
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 560" width="1280" height="560">
<title>Lintel</title>
<defs><radialGradient id="glow"><stop offset="0" stop-color="#6b86ea" stop-opacity="{0.38 if dark else 0.28}"/><stop offset="1" stop-color="#6b86ea" stop-opacity="0"/></radialGradient></defs>
<ellipse cx="300" cy="280" rx="300" ry="250" fill="url(#glow)"/>
<g transform="translate(130,110) scale(0.332)">{inner}</g>
<text x="516" y="262" font-size="132" font-weight="700" letter-spacing="-3" fill="{ink}" {font}>Lintel</text>
<text x="522" y="322" font-size="34" fill="{muted}" {font}>A menu bar for Windows</text>
{items}
</svg>
'''

for m in ("dark", "light"):
    open(f"{out}/banner-{m}.svg", "w", encoding="utf-8").write(banner(m))
