# Handoff: bottom-scroll-area fix (`fix/ui-bottom-scroll-gap`)

## Status

Fixed and verified. Branch `fix/ui-bottom-scroll-gap` (worktree
`.claude/worktrees/vibe-scroll-gap`), based on main @ 3d92922.

## The bug

In the real Tauri webview (WebKitGTK), wheeling an inner pane (article list, reader, …) past its
bottom chained the scroll to an outer scroller and the whole app shifted up, revealing a large
empty area beneath (screenshots `real-wheel20.png` / `real-up2.png` in `/tmp/repro-scroll/`).
Plain-browser WebKit (playwright MiniBrowser) never reproduced it: the document never overflows
there, so the chaining target only exists in the embedded webview.

## The fix (commit 2)

- `src/app.css`: `html, body { overflow: hidden; overscroll-behavior: none; }` — the shell is
  exactly 100% and clips its own panes, so the document must never scroll. The viewport's
  overscroll behavior is taken from the root element, so it must be set on `html` (it was only on
  `body`).
- `overscroll-behavior-y: none` on every inner scroller — `.viewport` (VirtualRows),
  `.reader` (Reader), `.scroll` (Sidebar), `.dialog` (Dialog) — so scrolling stops at the pane
  instead of chaining to any ancestor.

## Verification

- Real webview: rebuilt the debug binary with `TAURI_CONFIG` devUrl → worktree vite on port 1430,
  ran it on the perf fixture library (`/tmp/feedrico-run2`), drove it with xdotool on :0. After 150
  wheel-downs past the list bottom the shell stays put, last row flush at the bottom —
  `/tmp/repro-scroll/fix-list-bottom.png`. Before the fix the same action shifted the app up with a
  big empty area (`real-wheel20.png`). The user also confirmed in their own running app.
- `npm run check`: svelte-check 0 errors / 0 warnings. `npm test`: 18 files, 135 tests passed.
  `cargo fmt --all -- --check`: clean. `cargo clippy`/`cargo test --workspace` not run — the change
  is frontend CSS only; `cargo build` succeeds.

## Notes

- Earlier `NOTES.md` (commit 1) parked the investigation with a wrong root-cause hypothesis; it is
  replaced by this handoff.
- Repro scripts/screenshots live in `/tmp/repro-scroll/` (session-local, not committed).
- No Rust changes; the user's root checkout and its port-1420 dev server were never touched.
