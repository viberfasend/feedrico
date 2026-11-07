"""Gossamer motif: drifting silk threads and dew, coloured by the app's tokens.

Every motif SVG colours itself with the CSS custom properties the app and site already
define, with the light-theme value as a fallback for <img> use:
  threads  var(--color-motif-gossamer)   dew  var(--color-unread-indicator)
"""
from geometry import ribbon, bezier, cubic, fmt

SILK = 'fill:var(--color-motif-gossamer,#D6C7AE)'
HAIR = 'fill:none;stroke:var(--color-motif-gossamer,#D6C7AE);stroke-linecap:round'
DEW = 'fill:var(--color-unread-indicator,#AB6510)'


def thread(p, width, peak=0.5, opacity=1.0, end=0.3):
    op = '' if opacity == 1 else f' opacity="{opacity}"'
    return f'<path style="{SILK}"{op} d="{ribbon(bezier(*p), width, peak, power=1.1, end=end, n=14)}"/>'


def hair(p, width=0.8, opacity=1.0):
    op = '' if opacity == 1 else f' opacity="{opacity}"'
    return f'<path style="{HAIR};stroke-width:{width}"{op} d="{cubic(p)}"/>'


def tail(p, length=30, lift=-0.6, width=0.6, opacity=1.0):
    """A loose end: leaves the thread along its tangent, then curls with the air."""
    import math
    (x2, y2), (x3, y3) = p[2], p[3]
    dx, dy = x3 - x2, y3 - y2
    L = math.hypot(dx, dy) or 1
    ux, uy = dx / L, dy / L
    nx, ny = -uy, ux
    a = (x3 + ux * length * 0.4, y3 + uy * length * 0.4)
    b = (x3 + ux * length * 0.8 + nx * length * lift * 0.3, y3 + uy * length * 0.8 + ny * length * lift * 0.3)
    c = (x3 + ux * length + nx * length * lift, y3 + uy * length + ny * length * lift)
    return hair(((x3, y3), a, b, c), width, opacity)


def dew(p, t, r, width, opacity=1.0):
    """A bead hanging just under the thread at parameter t."""
    x, y = bezier(*p)(t)
    op = '' if opacity == 1 else f' opacity="{opacity}"'
    return f'<circle style="{DEW}"{op} cx="{fmt(x)}" cy="{fmt(y + width * 0.3 + r * 0.6)}" r="{fmt(r)}"/>'


def doc(w, h, body, title):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" fill="none" aria-hidden="true">\n'
            f'<!-- {title} -->\n{body}\n</svg>\n')


def single():
    p = ((4, 30), (120, 96), (300, 84), (440, 26))
    return doc(480, 96, '\n'.join([
        thread(p, 2.6, 0.42),
        tail(p, 34, 0.5),
        dew(p, 0.36, 3.0, 2.6), dew(p, 0.58, 2.0, 2.6),
    ]), 'Gossamer: one thread, sagging between two points')


def drifting():
    a = ((0, 206), (340, 30), (720, 280), (1130, 46))
    b = ((0, 70), (400, 230), (780, -20), (1200, 132))
    c = ((180, 240), (520, 150), (840, 220), (1200, 190))
    return doc(1200, 240, '\n'.join([
        thread(c, 1.4, 0.6, 0.4),
        thread(b, 2.0, 0.35, 0.65),
        thread(a, 3.0, 0.45),
        tail(a, 60, -0.45, 0.7),
        dew(a, 0.27, 3.2, 3.0), dew(a, 0.47, 2.2, 3.0), dew(a, 0.8, 2.6, 3.0),
        dew(b, 0.66, 1.8, 2.0, 0.7),
    ]), 'Gossamer: drifting threads, lifted to the right by warm air')


def dew_drops():
    p = ((2, 6), (28, 18), (68, 18), (94, 6))
    return doc(96, 28, '\n'.join([
        hair(p, 0.9),
        dew(p, 0.3, 1.6, 0.9), dew(p, 0.5, 2.6, 0.9), dew(p, 0.7, 1.6, 0.9),
    ]), 'Gossamer: dew on a strand, for dividers and small accents')


def corner():
    p = ((0, 0), (40, 120), (120, 190), (214, 208))
    q = ((0, 0), (90, 60), (150, 130), (236, 150))
    return doc(240, 240, '\n'.join([
        thread(q, 1.6, 0.3, 0.6),
        thread(p, 2.4, 0.3),
        tail(p, 26, 0.6),
        dew(p, 0.45, 2.8, 2.4), dew(q, 0.6, 1.6, 1.6, 0.7),
    ]), 'Gossamer: a thread anchored in the top-left corner (mirror for other corners)')


def write_all(write):
    import geometry
    geometry.PRECISION = 1
    write('motif/gossamer-thread.svg', single())
    write('motif/gossamer-drifting.svg', drifting())
    write('motif/gossamer-dew.svg', dew_drops())
    write('motif/gossamer-corner.svg', corner())
    geometry.PRECISION = 2
