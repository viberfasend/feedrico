"""The Feedrico mark: the feed symbol spun from silk.

A drop of honey light (the feed dot) with two threads of late-summer gossamer arcing around
it (the feed waves). Coordinates live on a 100-unit grid; the drawing spans 16..84.
"""
from geometry import ribbon, arc, cubic, fmt

DROP = (28, 72)
DROP_R = 10.5

# (radius, start angle, end angle, width, peak, wave)
THREADS_FULL = [
    (32, 100, -10, 4.4, 0.46, 0.5),
    (53, 103, -13, 5.0, 0.44, 0.8),
]
# the logo weight: matches the stems of the wordmark
THREADS_LOGO = [
    (32, 99, -9, 7.4, 0.46, 0.4),
    (53, 101, -11, 7.8, 0.44, 0.6),
]
THREADS_SMALL = [
    (32, 95, -5, 11.5, 0.45, 0),
    (53, 96, -6, 12, 0.42, 0),
]
# dew beads: (thread index, t along thread, radius)
BEADS = [(1, 0.27, 2.2), (1, 0.66, 1.6), (0, 0.52, 1.5)]


def threads(small=False, weight=None):
    spec = {'small': THREADS_SMALL, 'logo': THREADS_LOGO, 'fine': THREADS_FULL}[weight or ('small' if small else 'fine')]
    power = 1.25 if spec is THREADS_FULL else 1.0
    return [ribbon(arc(DROP[0], DROP[1], r, a0, a1, wave), w, peak, power=power, n=12) for r, a0, a1, w, peak, wave in spec]


def beads():
    out = []
    for i, t, r in BEADS:
        rr, a0, a1, w, peak, wave = THREADS_FULL[i]
        x, y = arc(DROP[0], DROP[1], rr, a0, a1, wave)(t)
        # dew hangs: pull each bead below the thread by gravity
        out.append((fmt(x), fmt(y + w * 0.35 + r * 0.55), fmt(r)))
    return out


def tails():
    """Loose hairline ends drifting off the outer thread (large sizes only)."""
    r, a0, a1, w, peak, wave = THREADS_FULL[1]
    c = arc(DROP[0], DROP[1], r, a0, a1, wave)
    sx, sy = c(0); ex, ey = c(1)
    # leave each end along its own tangent, then let the air lift it
    top = cubic(((sx, sy), (sx - 8, sy + 1.6), (sx - 14, sy - 1), (sx - 20, sy - 9)))
    bottom = cubic(((ex, ey), (ex - 1.6, ey + 7.5), (ex + 1, ey + 13), (ex + 7, ey + 19)))
    return top, bottom


def group(thread_fill, drop_fill, bead_fill=None, small=False, with_tails=False, tail_stroke=None,
          tail_width=0.5, drop_extra='', weight=None):
    parts = [f'<path fill="{thread_fill}" d="{d}"/>' for d in threads(small, weight)]
    if with_tails:
        for d in tails():
            parts.append(f'<path fill="none" stroke="{tail_stroke or thread_fill}" '
                         f'stroke-width="{tail_width}" stroke-linecap="round" d="{d}"/>')
    parts.append(f'<circle cx="{DROP[0]}" cy="{DROP[1]}" r="{DROP_R + (1 if small or weight == 'logo' else 0)}" fill="{drop_fill}"/>')
    parts.append(drop_extra)
    if bead_fill and not small:
        parts += [f'<circle cx="{x}" cy="{y}" r="{r}" fill="{bead_fill}"/>' for x, y, r in beads()]
    return '\n'.join(p for p in parts if p)
