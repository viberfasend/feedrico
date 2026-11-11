/** What a reading pane needs for paging; an HTMLElement fits. */
export interface Scroller {
  scrollTop: number;
  clientHeight: number;
  scrollHeight: number;
}

/** Lines kept in view when paging, so the eye finds its place again. */
const OVERLAP_PX = 40;

/**
 * Space / Shift+Space in the reader: scrolls one screen and reports `'end'` when a forward page
 * is already at the bottom (the caller then moves to the next Article). Paging back at the top
 * just stays. Jumps without animation, so it needs no reduced-motion case.
 */
export function pageReader(scroller: Scroller, forward: boolean): 'scrolled' | 'end' {
  const step = Math.max(scroller.clientHeight - OVERLAP_PX, OVERLAP_PX);
  const bottom = scroller.scrollHeight - scroller.clientHeight;
  if (forward) {
    if (scroller.scrollTop >= bottom - 1) return 'end';
    scroller.scrollTop = Math.min(scroller.scrollTop + step, bottom);
  } else {
    scroller.scrollTop = Math.max(scroller.scrollTop - step, 0);
  }
  return 'scrolled';
}
