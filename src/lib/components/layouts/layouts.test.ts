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
});
