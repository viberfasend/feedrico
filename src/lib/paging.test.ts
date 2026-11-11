import { describe, expect, it } from 'vitest';
import { pageReader } from './paging';

const pane = (scrollTop: number) => ({ scrollTop, clientHeight: 600, scrollHeight: 2000 });

describe('pageReader (Space in the reader)', () => {
  it('scrolls one screen forward, keeping a little overlap', () => {
    const el = pane(0);
    expect(pageReader(el, true)).toBe('scrolled');
    expect(el.scrollTop).toBe(560);
  });

  it('stops at the bottom, then reports the end so the next Article opens', () => {
    const el = pane(1200);
    expect(pageReader(el, true)).toBe('scrolled');
    expect(el.scrollTop).toBe(1400);
    expect(pageReader(el, true)).toBe('end');
  });

  it('reports the end at once for an Article shorter than the pane', () => {
    expect(pageReader({ scrollTop: 0, clientHeight: 600, scrollHeight: 400 }, true)).toBe('end');
  });

  it('scrolls back and stays at the top', () => {
    const el = pane(300);
    expect(pageReader(el, false)).toBe('scrolled');
    expect(el.scrollTop).toBe(0);
    expect(pageReader(el, false)).toBe('scrolled');
  });
});
