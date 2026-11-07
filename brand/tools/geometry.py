"""Shared geometry for the Feedrico mark and motif: tapered silk threads and dew drops.

A thread is a ribbon around a centre curve. Its half-width swells from zero at both ends
to a peak, like a strand of spider silk that catches the light along part of its length.
"""
import math


PRECISION = 2   # decimals in emitted path data; motif files use 1


def fmt(v):
    s = f"{v:.{PRECISION}f}".rstrip('0').rstrip('.')
    return '0' if s in ('-0', '') else s


def _smooth(pts, closed=False):
    """Catmull-Rom through pts, emitted as cubic Bezier commands (without the initial M)."""
    out = []
    n = len(pts)
    for i in range(n - 1):
        p0 = pts[i - 1] if i > 0 else pts[i]
        p1, p2 = pts[i], pts[i + 1]
        p3 = pts[i + 2] if i + 2 < n else pts[i + 1]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
        out.append(f"C{fmt(c1[0])} {fmt(c1[1])} {fmt(c2[0])} {fmt(c2[1])} {fmt(p2[0])} {fmt(p2[1])}")
    return ''.join(out)


def ribbon(curve, width, peak=0.5, n=14, power=0.85, end=0.0):
    """Closed path around curve(t) for t in [0, 1].

    width: maximum thickness; peak: where along the thread it is thickest;
    end: thickness kept at the very ends (0 = needle-sharp).
    """
    k = math.log(0.5) / math.log(peak)
    centre, normal, half = [], [], []
    for i in range(n + 1):
        t = i / n
        x, y = curve(t)
        e = 1e-4
        xa, ya = curve(max(0, t - e)); xb, yb = curve(min(1, t + e))
        dx, dy = xb - xa, yb - ya
        L = math.hypot(dx, dy) or 1
        centre.append((x, y)); normal.append((-dy / L, dx / L))
        h = (width / 2) * (math.sin(math.pi * (t ** k)) ** power)
        half.append(max(h, end / 2))
    left = [(c[0] + nm[0] * h, c[1] + nm[1] * h) for c, nm, h in zip(centre, normal, half)]
    right = [(c[0] - nm[0] * h, c[1] - nm[1] * h) for c, nm, h in zip(centre, normal, half)]
    right.reverse()
    d = f"M{fmt(left[0][0])} {fmt(left[0][1])}" + _smooth(left)
    d += f"L{fmt(right[0][0])} {fmt(right[0][1])}" + _smooth(right) + "Z"
    return d


def arc(cx, cy, r, a0, a1, wave=0.0, waves=1.0):
    """Circular arc around (cx, cy), angles in degrees counter-clockwise from +x (y points down)."""
    def f(t):
        a = math.radians(a0 + (a1 - a0) * t)
        rr = r + wave * math.sin(math.pi * 2 * waves * t)
        return (cx + rr * math.cos(a), cy - rr * math.sin(a))
    return f


def bezier(*p):
    """Cubic Bezier through four control points."""
    def f(t):
        u = 1 - t
        return (u**3 * p[0][0] + 3 * u * u * t * p[1][0] + 3 * u * t * t * p[2][0] + t**3 * p[3][0],
                u**3 * p[0][1] + 3 * u * u * t * p[1][1] + 3 * u * t * t * p[2][1] + t**3 * p[3][1])
    return f


def at(curve, t):
    x, y = curve(t)
    return fmt(x), fmt(y)


def polyline(curve, n=24):
    pts = [curve(i / n) for i in range(n + 1)]
    return f"M{fmt(pts[0][0])} {fmt(pts[0][1])}" + _smooth(pts)


def cubic(p):
    """A cubic Bezier as plain path data."""
    return 'M' + ' '.join(f"{fmt(x)} {fmt(y)}" if i != 1 else f"C{fmt(x)} {fmt(y)}" for i, (x, y) in enumerate(p))
