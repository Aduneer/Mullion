# Draws an app's README banners, assets/banner-dark.svg and banner-light.svg, from its logo
# (assets/<app>.svg): the logo, the name and what it is, and a rule in the app's own color.
#   python -I tools/banner.py apps/lintel
import os, re, sys

# Each app's name, line and color (its logo's tile). Mullion's banner also lists the apps.
APPS = {
    "mullion": ("Mullion", "Well-made parts for the Windows desktop", "#1c1b19"),
    "oriel": ("Oriel", "A command centre for your second screen", "#2e6e60"),
    "lintel": ("Lintel", "A menu bar for Windows", "#b84d35"),
    "fanlight": ("Fanlight", "Search and launch for Windows", "#dd9d2c"),
    "bay": ("Bay", "Every window at once, for Windows", "#2f5d8c"),
    "sash": ("Sash", "Window snapping for Windows", "#6b4c8a"),
    "mitre": ("Mitre", "Hot corners for Windows", "#8b5e34"),
}

folder = sys.argv[1]
app = os.path.basename(os.path.normpath(folder))
name, line, color = APPS[app]
logo = open(os.path.join(folder, "assets", f"{app}.svg"), encoding="utf-8").read()
inner = re.search(r"<svg[^>]*>(.*)</svg>", logo, re.S).group(1)
inner = re.sub(r"<title>.*?</title>|<!--.*?-->", "", inner, flags=re.S)

def banner(mode):
    dark = mode == "dark"
    ink, muted, chip, edge = (("#f0efec", "#898781", "#1f1e1c", "rgba(240,239,236,0.12)") if dark
                              else ("#1c1b19", "#6b6a66", "#ffffff", "rgba(28,27,25,0.12)"))
    # Mullion's own color is ink, which a dark page hides: its rule is the paper color there.
    rule = ink if color == "#1c1b19" else color
    font = 'font-family="Segoe UI Variable Display, Segoe UI, system-ui, -apple-system, sans-serif"'
    small = 'font-family="Segoe UI Variable Text, Segoe UI, system-ui, -apple-system, sans-serif"'
    extra = ""
    if app == "mullion":
        x = 522
        for other in ("oriel", "lintel", "fanlight", "bay", "sash", "mitre"):
            n, _, c = APPS[other]
            w = 36 + len(n) * 9.6
            extra += f'''
<rect x="{x}" y="380" width="{w:.0f}" height="40" rx="10" fill="{chip}" stroke="{edge}"/>
<rect x="{x + 14}" y="393" width="14" height="14" rx="4" fill="{c}"/>
<text x="{x + 36}" y="406" font-size="16" fill="{ink}" {small}>{n}</text>'''
            x += w + 12
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 560" width="1280" height="560">
<title>{name}</title>
<g transform="translate(140,120) scale(0.3125)">{inner}</g>
<text x="516" y="262" font-size="132" font-weight="700" letter-spacing="-3" fill="{ink}" {font}>{name}</text>
<rect x="522" y="292" width="72" height="8" rx="4" fill="{rule}"/>
<text x="522" y="352" font-size="34" fill="{muted}" {font}>{line}</text>{extra}
</svg>
'''

for m in ("dark", "light"):
    with open(os.path.join(folder, "assets", f"banner-{m}.svg"), "w", encoding="utf-8", newline="\n") as f:
        f.write(banner(m))
