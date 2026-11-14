// Pane sizing as pure functions: bounds, clamping and keyboard steps for the splitters.
// Widths are machine-local view preferences (spec: "Panes"); the Library never sees them.

export interface PaneLimits {
  sidebarMin: number;
  sidebarMax: number;
  listMin: number;
  listMax: number;
  readerMin: number;
}

export interface PaneInput {
  /** Preferred widths in px; they survive a window shrink and come back when it grows. */
  sidebar: number;
  list: number;
  sidebarHidden: boolean;
  /** Grid Layouts have no Article list column: the grid takes the rest. */
  grid: boolean;
  /** Window width in px. */
  available: number;
}

export const STEP = 16;
export const LARGE_STEP = 64;

export const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), Math.max(min, max));

/**
 * The widths actually used. The reader keeps at least `readerMin`; when the window is too
 * narrow the Article list gives way first (down to its minimum), then the sidebar.
 */
export function layoutPanes(input: PaneInput, limits: PaneLimits): { sidebar: number; list: number } {
  const budget = input.available - limits.readerMin;
  let sidebar = input.sidebarHidden ? 0 : clamp(input.sidebar, limits.sidebarMin, limits.sidebarMax);
  if (input.grid) {
    if (!input.sidebarHidden) sidebar = clamp(sidebar, limits.sidebarMin, budget);
    return { sidebar, list: 0 };
  }
  let list = clamp(input.list, limits.listMin, limits.listMax);
  if (sidebar + list > budget) list = Math.max(limits.listMin, budget - sidebar);
  if (sidebar + list > budget && !input.sidebarHidden) {
    sidebar = Math.max(limits.sidebarMin, budget - list);
  }
  return { sidebar, list };
}

/** Range the sidebar splitter can reach. The list may shrink to its minimum to make room. */
export function sidebarRange(grid: boolean, available: number, limits: PaneLimits) {
  const room = available - limits.readerMin - (grid ? 0 : limits.listMin);
  return { min: limits.sidebarMin, max: Math.max(limits.sidebarMin, Math.min(limits.sidebarMax, room)) };
}

/** Range the Article list splitter can reach, given the sidebar's current width. */
export function listRange(sidebar: number, available: number, limits: PaneLimits) {
  const room = available - limits.readerMin - sidebar;
  return { min: limits.listMin, max: Math.max(limits.listMin, Math.min(limits.listMax, room)) };
}

/** Range of the slide-over reader: at least the reader minimum, at most `maxFraction` of the window. */
export function slideOverRange(available: number, maxFraction: number, limits: PaneLimits) {
  return {
    min: limits.readerMin,
    max: Math.max(limits.readerMin, Math.floor(available * maxFraction)),
  };
}

/**
 * The new value for a splitter key, or null when the key isn't a resize key. Arrows move the
 * separator itself, so `inverted` splitters (the slide-over's leading edge) grow on ←.
 */
export function stepForKey(
  key: string,
  shift: boolean,
  value: number,
  min: number,
  max: number,
  inverted = false,
): number | null {
  const step = shift ? LARGE_STEP : STEP;
  switch (key) {
    case 'ArrowRight':
      return clamp(value + (inverted ? -step : step), min, max);
    case 'ArrowLeft':
      return clamp(value + (inverted ? step : -step), min, max);
    case 'Home':
      return min;
    case 'End':
      return max;
    default:
      return null;
  }
}

export interface StoredPanes {
  sidebar: number | null;
  list: number | null;
  slideOver: number | null;
  sidebarHidden: boolean;
}

/** Tolerant parse of the stored JSON: anything unexpected falls back to the defaults. */
export function parseStoredPanes(raw: string | null): StoredPanes {
  let data: Record<string, unknown> = {};
  try {
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    if (parsed && typeof parsed === 'object') data = parsed as Record<string, unknown>;
  } catch {
    // corrupt value: use defaults
  }
  const num = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : null);
  return {
    sidebar: num(data.sidebar),
    list: num(data.list),
    slideOver: num(data.slideOver),
    sidebarHidden: data.sidebarHidden === true,
  };
}
