import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// Stacking and native controls can't be seen in happy-dom (no layout, no painting), so these
// guard the CSS itself.

const base = readFileSync(new URL('../tokens/generated/base.css', import.meta.url), 'utf8');
const components = import.meta.glob(['../../**/*.svelte'], { query: '?raw', import: 'default', eager: true }) as Record<
  string,
  string
>;

const component = (name: string) => {
  const entry = Object.entries(components).find(([file]) => file.endsWith(`/ui/${name}`) || file === `./${name}`);
  if (!entry) throw new Error(`${name} not found`);
  return entry[1];
};

const layer = (name: string) => {
  const match = new RegExp(`--layer-${name}:\\s*(\\d+)`).exec(base);
  if (!match) throw new Error(`--layer-${name} not found`);
  return Number(match[1]);
};

/** The declarations of the first rule for `selector` in `source`. */
const rule = (source: string, selector: string) => {
  const start = source.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`${selector} not found`);
  return source.slice(start, source.indexOf('}', start));
};

describe('Stacking layers', () => {
  // Bug (Settings and Add Feed unusable): the dialog scrim had z-index 20 and the dialog none,
  // so the scrim was painted over the dialog and took every click.
  it('paints dialogs above their scrim, and menus and tooltips above dialogs', () => {
    expect(layer('dialog')).toBeGreaterThan(layer('scrim'));
    expect(layer('popover')).toBeGreaterThan(layer('dialog'));
    expect(layer('toast')).toBeGreaterThan(layer('dialog'));
    expect(layer('scrim')).toBeGreaterThan(layer('slide-over'));
    expect(layer('slide-over')).toBeGreaterThan(layer('backdrop'));
  });

  it('puts every overlay on its layer', () => {
    const dialog = component('Dialog.svelte');
    expect(rule(dialog, '.scrim')).toMatch(/z-index:\s*var\(--layer-scrim\)/);
    expect(rule(dialog, '.dialog')).toMatch(/z-index:\s*var\(--layer-dialog\)/);
    for (const [file, selector] of [
      ['Menu.svelte', '.menu'],
      ['ContextMenu.svelte', '.menu'],
      ['Tooltip.svelte', '.tooltip'],
      ['IconButton.svelte', '.tooltip'],
    ] as const) {
      expect(rule(component(file), selector), `${file} ${selector}`).toMatch(/z-index:\s*var\(--layer-popover\)/);
    }
  });

  it('uses the layer tokens for every z-index, so the order lives in one place', () => {
    expect(Object.keys(components).length, 'the glob found the components').toBeGreaterThan(20);
    const raw = Object.entries(components)
      .flatMap(([file, source]) => [...source.matchAll(/z-index:\s*([^;]+);/g)].map((m) => `${file}: ${m[1]}`))
      .filter((entry) => !/var\(--layer-[a-z-]+\)$/.test(entry));
    expect(raw).toEqual([]);
  });
});
