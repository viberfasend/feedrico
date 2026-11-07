"""App icon: the mark on a dusk tile lit by a low honey sun, on the macOS 1024 grid."""
import mark
from geometry import fmt

C = {
    'dusk950': '#15121F', 'dusk900': '#1E1A2F', 'dusk800': '#2B2541', 'dusk700': '#3F365B',
    'dusk600': '#544878',
    'paper0': '#FFFDF9', 'paper50': '#FBF7EF', 'paper100': '#F4EDE0',
    'amber50': '#FFF6E2', 'amber100': '#FDE9BF', 'amber200': '#F9D386', 'amber300': '#F3B84B',
    'amber400': '#E89C26', 'amber500': '#CF8015', 'amber600': '#AB6510',
}


def tile_defs(prefix, scale, ox, oy):
    dx, dy = mark.DROP
    gx, gy = ox + dx * scale, oy + dy * scale
    return f'''  <linearGradient id="{prefix}dusk" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="{C['dusk800']}"/>
      <stop offset="1" stop-color="{C['dusk900']}"/>
    </linearGradient>
    <radialGradient id="{prefix}glow" gradientUnits="userSpaceOnUse" cx="{fmt(gx)}" cy="{fmt(gy)}" r="{fmt(scale * 26)}">
      <stop offset="0.38" stop-color="{C['amber300']}" stop-opacity="0.34"/>
      <stop offset="0.55" stop-color="{C['amber400']}" stop-opacity="0.12"/>
      <stop offset="1" stop-color="{C['amber400']}" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="{prefix}drop" cx="0.42" cy="0.4" r="0.62">
      <stop offset="0" stop-color="{C['amber200']}"/>
      <stop offset="0.7" stop-color="{C['amber300']}"/>
      <stop offset="1" stop-color="{C['amber400']}"/>
    </radialGradient>'''


def app_icon(size_class='large'):
    """'large': the 1024 master on the macOS grid; 'small': 16-48 px renders;
    'touch': full-bleed opaque square for apple-touch-icon and maskable web icons."""
    small = size_class == 'small'
    touch = size_class == 'touch'
    if touch:
        x0, w, rx = 0, 1024, 0
        scale = 8.0
    elif small:
        x0, w, rx = 32, 960, 210     # nearly full-bleed: small icons need every pixel
        scale = 10.8
    else:
        x0, w, rx = 100, 824, 185    # macOS grid: 824 tile, ~185 corner radius
        scale = 7.0
    cx = x0 + w / 2
    ox, oy = cx - 50 * scale + 6 * scale * 0, cx - 50 * scale
    defs = tile_defs('', scale, ox, oy)
    body = mark.group(C['paper50'], 'url(#drop)', bead_fill=C['amber200'], small=small,
                      with_tails=not small and not touch, tail_stroke=C['paper50'], tail_width=0.45)
    shadow = '' if small or touch else f'''
  <rect x="{x0}" y="{x0 + 14}" width="{w}" height="{w}" rx="{rx}" fill="{C['dusk950']}" opacity="0.28" filter="url(#soft)"/>'''
    filt = '' if small or touch else '''
    <filter id="soft" x="-10%" y="-10%" width="120%" height="120%"><feGaussianBlur stdDeviation="14"/></filter>
    <clipPath id="tile"><rect x="100" y="100" width="824" height="824" rx="185"/></clipPath>'''
    clip = '' if small or touch else ' clip-path="url(#tile)"'
    edge = '' if touch else (f'  <rect x="{x0 + 2}" y="{x0 + 2}" width="{w - 4}" height="{w - 4}" rx="{rx - 2}" '
                             f'fill="none" stroke="{C["paper50"]}" stroke-opacity="0.09" stroke-width="4"/>\n')
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <title>Feedrico</title>
  <defs>
  {defs}{filt}
  </defs>{shadow}
  <rect x="{x0}" y="{x0}" width="{w}" height="{w}" rx="{rx}" fill="url(#dusk)"/>
  <rect x="{x0}" y="{x0}" width="{w}" height="{w}" rx="{rx}" fill="url(#glow)"/>
  <g{clip}>
  <g transform="translate({fmt(ox)} {fmt(oy)}) scale({scale})">
{body}
  </g>
  </g>
{edge}</svg>
'''
