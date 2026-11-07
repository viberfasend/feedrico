"""Outline text to SVG path data: HarfBuzz shaping (kerning) + fontTools outlines.

Reads the bundled Literata and Inter from node_modules/@fontsource-variable (OFL).
"""
import uharfbuzz as hb
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.boundsPen import BoundsPen

import os
FONTS = os.path.join(os.path.dirname(__file__), '..', '..', 'node_modules', '@fontsource-variable') + os.sep
PATHS = {
    'literata': FONTS + 'literata/files/literata-latin-opsz-normal.woff2',
    'literata-italic': FONTS + 'literata/files/literata-latin-opsz-italic.woff2',
    'inter': FONTS + 'inter/files/inter-latin-opsz-normal.woff2',
}
_cache = {}

def _load(name):
    if name not in _cache:
        tt = TTFont(PATHS[name])
        import io
        buf = io.BytesIO(); tt.flavor = None; tt.save(buf)
        data = buf.getvalue()
        face = hb.Face(data)
        _cache[name] = (TTFont(io.BytesIO(data)), face)
    return _cache[name]

PRECISION = 2


def fmt(v):
    s = f"{v:.{PRECISION}f}".rstrip('0').rstrip('.')
    return '0' if s == '-0' else s

class RoundPen(SVGPathPen):
    def __init__(self, gs):
        super().__init__(gs, ntos=fmt)

def outline(text, font='literata', size=100, x=0, y=0, axes=None, features=None, tracking=0):
    """Return (d, advance, glyphs[]) where each glyph has name, x, bounds (in output coords)."""
    tt, face = _load(font)
    upem = tt['head'].unitsPerEm
    hbfont = hb.Font(face)
    if axes:
        hbfont.set_variations(axes)
    buf = hb.Buffer(); buf.add_str(text); buf.guess_segment_properties()
    hb.shape(hbfont, buf, features or {"kern": True, "liga": True})
    gs = tt.getGlyphSet(location=axes or {})
    order = tt.getGlyphOrder()
    s = size / upem
    pen = RoundPen(gs)
    cx = 0
    glyphs = []
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        name = order[info.codepoint]
        gx = x + (cx + pos.x_offset) * s
        gy = y - pos.y_offset * s
        t = (s, 0, 0, -s, gx, gy)
        gs[name].draw(TransformPen(pen, t))
        bp = BoundsPen(gs); gs[name].draw(bp)
        b = bp.bounds
        if b:
            b = (gx + b[0]*s, gy - b[3]*s, gx + b[2]*s, gy - b[1]*s)
        glyphs.append({'name': name, 'x': gx, 'bounds': b, 'adv': pos.x_advance*s})
        cx += pos.x_advance + tracking * upem / 1000
    return pen.getCommands(), cx * s, glyphs

def metrics(font='literata', size=100, axes=None):
    tt, _ = _load(font)
    s = size / tt['head'].unitsPerEm
    os2 = tt['OS/2']
    return {'cap': os2.sCapHeight*s, 'x': os2.sxHeight*s, 'asc': tt['hhea'].ascent*s, 'desc': tt['hhea'].descent*s}
