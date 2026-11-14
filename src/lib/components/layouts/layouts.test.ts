import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const css = (file: string) => readFileSync(new URL(`../../tokens/generated/${file}`, import.meta.url), 'utf8');
const value = (source: string, name: string) => {
  const match = new RegExp(`${name}:\\s*([\\d.]+)`).exec(source);
  if (!match) throw new Error(`${name} not found`);
  return Number(match[1]);
};

const sources = import.meta.glob('./*.svelte', { query: '?raw', import: 'default', eager: true });

describe('Layout components', () => {
  // Bug (Gallery tiles stayed empty): WebKitGTK never starts lazy image loads inside the
  // virtualized scroller, and fires no error either. Virtualization already limits images to
  // visible rows, so lazy loading is never needed here.
  it.each(Object.entries(sources))('%s does not lazy-load images', (_name, source) => {
    expect(source).not.toMatch(/loading=["']lazy["']/);
  });

  it('found the Layout components', () => {
    expect(Object.keys(sources).length).toBeGreaterThanOrEqual(4);
  });

  // Bug (#59): the comfortable List row was 76 px, too short for meta, title and excerpt, so the
  // excerpt line lost its descenders. The row must fit all three lines plus padding.
  it('gives a comfortable List row room for meta, title and a full excerpt line', () => {
    const base = css('base.css');
    const comfortable = css('density-comfortable.css');
    const ui = value(base, '--font-line-height-ui');
    const lines =
      (value(base, '--font-size-caption') + value(base, '--font-size-body') + value(base, '--font-size-small')) * 16 * ui;
    const needed =
      lines + 2 * value(base, '--space-stack-2xs') + 2 * value(base, '--space-inset-md') + 1;
    expect(value(comfortable, '--article-list-row-height')).toBeGreaterThanOrEqual(Math.ceil(needed));
  });

  // #104: titles wrap to two lines inside the fixed row, so the row must fit meta, a two-line
  // title and one excerpt line, and compact must fit meta plus a two-line title.
  it('gives List rows room for a two-line title in both densities', () => {
    const base = css('base.css');
    const ui = value(base, '--font-line-height-ui');
    const px = (name: string) => value(base, name) * 16;
    const meta = px('--font-size-caption') * ui;
    const title = 2 * px('--font-size-body') * ui;
    const excerpt = px('--font-size-small') * ui;
    const comfortable =
      meta + title + excerpt + 2 * value(base, '--space-stack-2xs') + 2 * value(base, '--space-inset-md') + 1;
    const compact = meta + title + value(base, '--space-stack-2xs') + 2 * value(base, '--space-inset-sm') + 1;
    expect(value(css('density-comfortable.css'), '--article-list-row-height')).toBeGreaterThanOrEqual(
      Math.ceil(comfortable)
    );
    expect(value(css('density-compact.css'), '--article-list-row-height')).toBeGreaterThanOrEqual(Math.ceil(compact));
  });

  it('clamps the List title to two lines instead of nowrap, and exposes it as a tooltip', () => {
    const source = String(sources['./ListLayout.svelte']);
    const title = /\.title\s*\{[^}]*\}/.exec(source)?.[0] ?? '';
    expect(title).toMatch(/line-clamp:\s*2/);
    expect(title).not.toMatch(/nowrap/);
    expect(source).toContain('title={article.title}');
  });
});
