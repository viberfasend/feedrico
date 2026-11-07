import { describe, expect, it } from 'vitest';

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
});
