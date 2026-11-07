"""The Feedrico wordmark: Literata (opsz 60, wght 540) outlined, with two drawn details.

1. The tittle of the i is a drop of honey, the same drop as in the mark.
2. A single strand of silk hangs from the top of the d to that drop, the way gossamer
   drifts onto whatever stands still in late-summer light.
"""
from textpath import outline
from geometry import ribbon, bezier, fmt

AXES = {'opsz': 60, 'wght': 540}
SIZE = 100
TEXT = 'Feedrıco'   # dotless i: we draw our own tittle


def build():
    d, adv, glyphs = outline(TEXT, 'literata', SIZE, 0, 0, AXES)
    gi = next(g for g in glyphs if g['name'] == 'dotlessi')
    gd = next(g for g in glyphs if g['name'] == 'd')
    # Literata's own tittle sits 16.1 units right of the glyph origin, centred at y -68.8, r~7.6
    drop = (gi['x'] + 16.1, -69.4, 8.0)
    # anchor: the top of the d's ascender serif
    anchor = (gd['bounds'][2] - 12.5, gd['bounds'][1] + 1.2)
    sag = bezier(anchor, (anchor[0] + 22, anchor[1] + 13), (drop[0] - 30, drop[1] + 11), (drop[0] - 4.5, drop[1] - 2))
    thread = ribbon(sag, 1.5, peak=0.55, power=0.9, end=0.35, n=18)
    width = adv
    return {'letters': d, 'drop': drop, 'thread': thread, 'width': width,
            'top': gd['bounds'][1], 'baseline': 0, 'bottom': 1.4}
