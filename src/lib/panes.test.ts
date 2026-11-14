import { describe, expect, it } from 'vitest';
import {
  layoutPanes,
  listRange,
  parseStoredPanes,
  sidebarRange,
  slideOverRange,
  stepForKey,
  type PaneInput,
  type PaneLimits,
} from './panes';

const limits: PaneLimits = { sidebarMin: 200, sidebarMax: 420, listMin: 280, listMax: 640, readerMin: 420 };
const base: PaneInput = { sidebar: 260, list: 380, sidebarHidden: false, grid: false, available: 1280 };

describe('layoutPanes', () => {
  it('keeps the preferred widths when there is room', () => {
    expect(layoutPanes(base, limits)).toEqual({ sidebar: 260, list: 380 });
  });

  it('clamps preferences to the token bounds', () => {
    expect(layoutPanes({ ...base, sidebar: 50, list: 2000, available: 3000 }, limits)).toEqual({ sidebar: 200, list: 640 });
  });

  it('shrinks the Article list first when the window is narrow', () => {
    // 1000 - 420 = 580 for sidebar + list: list gives way to 320.
    expect(layoutPanes({ ...base, available: 1000 }, limits)).toEqual({ sidebar: 260, list: 320 });
  });

  it('then shrinks the sidebar, never below its minimum', () => {
    expect(layoutPanes({ ...base, available: 880 }, limits)).toEqual({ sidebar: 200, list: 280 });
  });

  it('gets the preferred widths back when the window grows again', () => {
    expect(layoutPanes({ ...base, available: 1600 }, limits)).toEqual({ sidebar: 260, list: 380 });
  });

  it('gives a hidden sidebar no width', () => {
    expect(layoutPanes({ ...base, sidebarHidden: true }, limits)).toEqual({ sidebar: 0, list: 380 });
  });

  it('has no list column in a grid Layout', () => {
    expect(layoutPanes({ ...base, grid: true }, limits)).toEqual({ sidebar: 260, list: 0 });
  });
});

describe('ranges', () => {
  it('limits the sidebar so the list can still reach its minimum', () => {
    expect(sidebarRange(false, 1280, limits)).toEqual({ min: 200, max: 420 });
    expect(sidebarRange(false, 1000, limits)).toEqual({ min: 200, max: 300 });
    expect(sidebarRange(true, 1000, limits)).toEqual({ min: 200, max: 420 });
    expect(sidebarRange(false, 880, limits).max).toBe(200);
  });

  it('limits the list so the reader keeps its minimum', () => {
    expect(listRange(260, 1280, limits)).toEqual({ min: 280, max: 600 });
    expect(listRange(260, 1920, limits)).toEqual({ min: 280, max: 640 });
    expect(listRange(260, 880, limits).max).toBe(280);
  });

  it('limits the slide-over to a share of the window', () => {
    expect(slideOverRange(1000, 0.88, limits)).toEqual({ min: 420, max: 880 });
    expect(slideOverRange(400, 0.88, limits)).toEqual({ min: 420, max: 420 });
  });
});

describe('stepForKey', () => {
  it('moves by 16 px, or 64 px with Shift, within the range', () => {
    expect(stepForKey('ArrowRight', false, 260, 200, 420)).toBe(276);
    expect(stepForKey('ArrowLeft', false, 260, 200, 420)).toBe(244);
    expect(stepForKey('ArrowRight', true, 260, 200, 420)).toBe(324);
    expect(stepForKey('ArrowLeft', true, 210, 200, 420)).toBe(200);
  });

  it('goes to the limits on Home and End', () => {
    expect(stepForKey('Home', false, 260, 200, 420)).toBe(200);
    expect(stepForKey('End', false, 260, 200, 420)).toBe(420);
  });

  it('grows on ArrowLeft when inverted', () => {
    expect(stepForKey('ArrowLeft', false, 720, 420, 900, true)).toBe(736);
    expect(stepForKey('ArrowRight', false, 720, 420, 900, true)).toBe(704);
  });

  it('ignores other keys', () => {
    expect(stepForKey('a', false, 260, 200, 420)).toBeNull();
  });
});

describe('parseStoredPanes', () => {
  it('reads valid values', () => {
    expect(parseStoredPanes('{"sidebar":300,"list":400,"slideOver":800,"sidebarHidden":true}')).toEqual({
      sidebar: 300,
      list: 400,
      slideOver: 800,
      sidebarHidden: true,
    });
  });

  it('falls back to defaults for missing, corrupt or odd values', () => {
    const defaults = { sidebar: null, list: null, slideOver: null, sidebarHidden: false };
    expect(parseStoredPanes(null)).toEqual(defaults);
    expect(parseStoredPanes('{nope')).toEqual(defaults);
    expect(parseStoredPanes('{"sidebar":"wide","list":-3,"sidebarHidden":1}')).toEqual(defaults);
  });
});
