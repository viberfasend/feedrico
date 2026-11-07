"""Build every Feedrico brand SVG from one source of geometry.

Run with a Python that has fonttools, brotli and uharfbuzz (for the outlined wordmark):
    uv run --with fonttools --with brotli --with uharfbuzz python brand/tools/build_brand.py
Outputs go straight into brand/. PNG exports are made by brand/tools/export.sh.
"""
import os
import mark, icon, wordmark, motif, social
from geometry import fmt

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), '..'))
C = icon.C
INK = C['dusk800']

SCHEMES = {
    '':          {'ink': INK, 'thread': INK, 'drop': C['amber400'], 'bead': C['amber500'], 'silk': C['amber500']},
    '-reversed': {'ink': C['paper50'], 'thread': C['paper50'], 'drop': C['amber300'], 'bead': C['amber200'], 'silk': C['amber200']},
    '-mono':     {'ink': 'currentColor', 'thread': 'currentColor', 'drop': 'currentColor', 'bead': 'currentColor', 'silk': 'currentColor'},
}


def write(rel, text):
    path = os.path.join(ROOT, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w') as f:
        f.write(text)


def svg(viewbox, body, title='Feedrico', extra=''):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{viewbox}" role="img" aria-label="{title}"{extra}>\n'
            f'<title>{title}</title>\n{body}\n</svg>\n')


def mark_body(s):
    return mark.group(s['thread'], s['drop'], weight='logo')


def wordmark_body(s, wm):
    x, y, r = wm['drop']
    return (f'<path fill="{s["ink"]}" d="{wm["letters"]}"/>\n'
            f'<path fill="{s["silk"]}" d="{wm["thread"]}"/>\n'
            f'<circle cx="{fmt(x)}" cy="{fmt(y)}" r="{fmt(r)}" fill="{s["drop"]}"/>')


def main():
    wm = wordmark.build()
    for suffix, s in SCHEMES.items():
        write(f'logo/feedrico-mark{suffix}.svg', svg('14 14 72 72', mark_body(s)))
        # wordmark: tight box with a little air
        top, w = wm['top'] - 2, wm['width']
        write(f'logo/feedrico-wordmark{suffix}.svg',
              svg(f'0 {fmt(top)} {fmt(w + 2)} {fmt(-top + 4)}', wordmark_body(s, wm)))
        # horizontal: mark spans from the d's ascender to just below the baseline
        mh = (-wm['top'] + 1.4) * 1.16
        sc = mh / 68
        my = 1.4 - 84 * sc + 0.2 * mh * 0      # align mark bottom (84) with the overshoot line
        gap = mh * 0.3
        mx = -16 * sc
        body = (f'<g transform="translate({fmt(mx)} {fmt(my)}) scale({fmt(sc)})">\n{mark_body(s)}\n</g>\n'
                f'<g transform="translate({fmt(68 * sc + gap)} 0)">\n{wordmark_body(s, wm)}\n</g>')
        total = 68 * sc + gap + w + 2
        write(f'logo/feedrico-lockup-horizontal{suffix}.svg',
              svg(f'0 {fmt(my + 16 * sc - 2)} {fmt(total)} {fmt(68 * sc + 4)}', body))
        # stacked: mark centred above the wordmark
        sc2 = 150 / 68
        mx2 = w / 2 - 50 * sc2
        my2 = wm['top'] - 34 - 84 * sc2
        body = (f'<g transform="translate({fmt(mx2)} {fmt(my2)}) scale({fmt(sc2)})">\n{mark_body(s)}\n</g>\n'
                f'{wordmark_body(s, wm)}')
        y0 = my2 + 16 * sc2 - 4
        write(f'logo/feedrico-lockup-stacked{suffix}.svg',
              svg(f'-4 {fmt(y0)} {fmt(w + 8)} {fmt(-y0 + 8)}', body))

    write('app-icon.svg', icon.app_icon('large'))
    write('app-icon-small.svg', icon.app_icon('small'))
    write('app-icon-touch.svg', icon.app_icon('touch'))
    write('favicon/favicon.svg', favicon())
    motif.write_all(write)
    import geometry, textpath
    geometry.PRECISION = 1
    textpath.PRECISION = 1
    write('social/og-card.svg', social.og_card(wm))


def favicon():
    body = mark.group('var(--t)', 'var(--d)', small=True).replace('fill="var(--t)"', 'class="t"').replace('fill="var(--d)"', 'class="d"')
    style = (f'<style>.t{{fill:{INK}}}.d{{fill:{C["amber400"]}}}'
             f'@media (prefers-color-scheme:dark){{.t{{fill:{C["paper100"]}}}.d{{fill:{C["amber300"]}}}}}</style>')
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="12 12 76 76">\n{style}\n{body}\n</svg>\n'


if __name__ == '__main__':
    main()
