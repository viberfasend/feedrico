"""Open Graph / social card, 1200x630: the mark at dusk, oversized and cropped, beside the promise."""
import mark
from icon import C
from textpath import outline
from geometry import fmt

HEADLINE = ['A calm reader for', 'the Feeds you follow.']
SUB = ['Everything lives in one Library folder on your', 'machine. No account, no sync, nothing phones home.']
FOOT = 'Desktop app for Linux, Windows and macOS'


def og_card(wm):
    scale = 6.6
    dx, dy = 905, 470                       # where the drop sits on the card
    ox, oy = dx - mark.DROP[0] * scale, dy - mark.DROP[1] * scale
    art = mark.group(C['paper50'], 'url(#ogdrop)', bead_fill=C['amber200'], with_tails=True,
                     tail_stroke=C['paper50'], tail_width=0.4)
    lines = []
    for i, t in enumerate(HEADLINE):
        d, _, _ = outline(t, 'literata', 66, 80, 268 + i * 80, {'opsz': 60, 'wght': 480})
        lines.append(f'<path fill="{C["paper50"]}" d="{d}"/>')
    for i, t in enumerate(SUB):
        d, _, _ = outline(t, 'inter', 25, 82, 420 + i * 38, {'opsz': 24, 'wght': 400})
        lines.append(f'<path fill="#C7BCDF" d="{d}"/>')
    d, _, _ = outline(FOOT, 'inter', 19, 82, 560, {'opsz': 20, 'wght': 500})
    lines.append(f'<path fill="#A699C8" d="{d}"/>')
    # small reversed lockup, top left
    s = 0.40
    from build_brand import mark_body, wordmark_body, SCHEMES
    rev = SCHEMES['-reversed']
    msc = 0.5
    lockup = (f'<g transform="translate(80 70)">'
              f'<g transform="translate({fmt(-16 * msc)} {fmt(-16 * msc)}) scale({msc})">{mark_body(rev)}</g>'
              f'<g transform="translate(46 32) scale({s})">{wordmark_body(rev, wm)}</g></g>')
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 630" width="1200" height="630">
<title>Feedrico: a calm reader for the Feeds you follow</title>
<defs>
  <radialGradient id="ogglow" gradientUnits="userSpaceOnUse" cx="{dx}" cy="{dy}" r="{fmt(scale * 30)}">
    <stop offset="0.3" stop-color="{C['amber300']}" stop-opacity="0.32"/>
    <stop offset="0.6" stop-color="{C['amber400']}" stop-opacity="0.1"/>
    <stop offset="1" stop-color="{C['amber400']}" stop-opacity="0"/>
  </radialGradient>
  <radialGradient id="ogdrop" cx="0.42" cy="0.4" r="0.62">
    <stop offset="0" stop-color="{C['amber200']}"/>
    <stop offset="0.7" stop-color="{C['amber300']}"/>
    <stop offset="1" stop-color="{C['amber400']}"/>
  </radialGradient>
</defs>
<rect width="1200" height="630" fill="{C['dusk900']}"/>
<rect width="1200" height="630" fill="url(#ogglow)"/>
<g transform="translate({fmt(ox)} {fmt(oy)}) scale({scale})">
{art}
</g>
{lockup}
{chr(10).join(lines)}
</svg>
'''
