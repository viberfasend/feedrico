# Notes: "empty area beneath when scrolled to the bottom" bug report

Status: INVESTIGATION STOPPED by the user ("you don't get it") — my root-cause analysis below is
UNVERIFIED and the user says it is wrong. No code was changed. Branch `fix/ui-bottom-scroll-gap`
is otherwise clean (worktree created from main @ 3d92922).

## What the user reported

Scrolling down: when the scroll container reaches the very bottom, there is empty area beneath
that can still be scrolled to — as if an inner scroller is at its bottom and an outer one then
scrolls down into the empty area.

## What I observed (real app, real webview)

- Ran my own instance: copied `target/debug/feedrico` to `/tmp/feedrico-run/feedrico` with the
  cargo perf fixture as `/tmp/feedrico-run/Feedrico Library` (portable mode picks it up), launched
  with `GDK_BACKEND=x11 DISPLAY=:0`, fresh XDG dirs, window 1280x820 at (1945,139), id 16777219.
  Drove it with xdotool; screenshots in `/tmp/repro-scroll/`.
- Clicked a small feed (Feed 84), wheeled the article list down past its bottom: the WHOLE app
  (sidebar, list header, footer) shifted up and a large empty area appeared beneath, persistently
  (`real-wheel20.png`, `real-up2.png`, `real-keyk.png`). Wheel-up did not restore it while the
  window had lost focus; with focus it scrolled the inner list but the outer offset stayed.
  => In the Tauri WebKitGTK 2.52 webview, an OUTER scroller (document/webview level) really does
  scroll once the inner `.viewport` hits its bottom. This part is observed, not inferred.
- Playwright WebKit (both cached webkit-2215 and webkit-2370 MiniBrowser) against the vite dev
  server on port 1430 with a mocked `window.__TAURI_INTERNALS__`: document NEVER overflows
  (`documentElement.scrollHeight == clientHeight`) and no chaining happens in any scenario
  (list/cards/magazine/gallery/slide-over/dialog/short window). Scripts: `/tmp/repro-scroll/repro*.mjs`.
  => The outer scroll only manifests in the Tauri-embedded webview, not in plain MiniBrowser, so
  the cause is likely in how the webview/document is sized or chained there — NOT in the pane CSS.
- Could not get DOM measurements from the running instance: `WEBKIT_INSPECTOR_SERVER=127.0.0.1:9229`
  listens but speaks no plain HTTP/WS I could drive; devtools console via xdotool was too fiddly.
  So WHICH element carries the outer scroll (document vs `.shell`/`.list` overflow:hidden) is UNKNOWN.

## My (unverified, probably wrong) hypothesis

- `src/app.css` sets `overscroll-behavior: none` only on `body`; the viewport's overscroll behavior
  comes from the root `html` element, and neither `html` nor `body` is `overflow: hidden`, so the
  document stays a scroll-chaining target in the webview. Proposed fix was: `html, body
  { overflow: hidden; overscroll-behavior: none }` + `overscroll-behavior-y: none` on inner
  scrollers. User says I don't get it — treat this as disproven, not just unverified.
- Secondary observation (measured, low confidence it matters): within-scroller trailing empty
  scroll space — reader `article` has `padding-bottom: calc(var(--reader-padding-block) * 3)`
  (72px) plus last-paragraph margin (~91px total, `s4-reader-bottom.png`); grid layouts leave
  `padding + gap` (~32px) after the last row because the `VirtualRows` spacer is
  `rowCount * rowHeight + 2 * padding` while each grid row's content is `gap` shorter.

## Environment / how to reproduce

- Worktree: `.claude/worktrees/vibe-scroll-gap` (branch `fix/ui-bottom-scroll-gap`), node_modules
  symlinked to `../../../node_modules`, `npm run tokens` run. Dev server was `npx vite --port 1430`
  (now stopped). Root checkout on main @ 3d92922, untouched; user's `tauri dev` on 1420 untouched.
- Repro scripts + screenshots: `/tmp/repro-scroll/` (repro.mjs/repro2/repro3/repro4.mjs,
  chain-test.mjs, real*.png, s3-*.png, s4-*.png). WebKit wrapper for playwright:
  `/tmp/repro-scroll/webkit-wrapper.sh` (LD_LIBRARY_PATH=/tmp/repro-scroll/webkit-libs + the
  bundle's own lib/sys dirs; system libs came from snaps).
- Mock for the browser repro: `window.__TAURI_INTERNALS__ = { invoke, transformCallback, ... }`
  in `page.addInitScript` (see repro.mjs) — the app runs in a plain browser with it.
- Real-instance driving: `xdotool` on DISPLAY :0 with `GDK_BACKEND=x11`; remember
  `mousemove --window <id> x y` is window-relative while plain `mousemove` is screen-absolute
  (window frame offset ~8,30); `windowactivate --sync` needed or events go nowhere.
