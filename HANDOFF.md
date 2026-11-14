# HANDOFF: Issue #80 — Design review: reader and images

## Status

**COMPLETE** — PR #132 created and pushed to origin/docs/review-reader-and-images.

## Issue

- **Number:** #80
- **Title:** Design review: reader and images
- **Type:** Documentation-only
- **Area:** docs/reviews

## What was built

Created **one new file**: `docs/reviews/2026-11-13-reader-and-images.md`

The document captures the design review from issue #80 as a durable record, containing:

- **Method:** research on established readers + mock render with `.claude/skills/debugging-feedrico/mock-reader.js` at 880 px minimum window, tested in Cards and List Layouts, both light and dark themes
- **Findings table:** 11 real-world Article scenarios with issues found on `main`:
  - WordPress post (72 px emoji images, share icons, tracking pixel)
  - Photo set (portrait photos past screen, no zoom)
  - Webcomic (hover-only tooltip, shrunk strip)
  - Summary-only news (Lead Image in card but not reader)
  - Podcast (raw file names, clicking attachment does nothing, no cover art)
  - Technical post (invisible diagram in dark theme, 16 px icon)
  - Offline images (dashed card with long label)
  - Long title/Feed name (wrapping issues)
  - Title only (empty page)
  - Chrome (actions scroll away, close button position, no prev/next)
- **What other readers do:** NetNewsWire, Feedbin, Miniflux, Reeder, Unread, NewsFlash, News Explorer
- **Decisions:**
  - Images treated by size alone (hidden ≤2 px, inline ≤32 px, emoji alt → text, block with caps)
  - Lead Image leads only when content has no picture
  - One Esc closes only the viewer
  - No inline audio/video player in v1; no image grids/carousels
- **Follow-up issues:** #81-#89 as filed from the review
- **Personas impact:** Skimmer, Archivist, Offline Reader, Newcomer, accessibility spectrum
- **Sources:** links to primary source repositories and documentation

## Deviations from brief

None. All requirements met:

- Created exactly ONE new file: `docs/reviews/2026-11-13-reader-and-images.md`
- Created the `docs/reviews/` directory
- No index file exists on main, so no index was modified
- Content is faithful to issue #80 body, restructured for readability
- No new findings invented
- Links to relevant spec sections in `docs/spec/v1.md` by anchor
- Read-only: no changes to src/**, crates/**, tokens/**, README.md, docs/spec/v1.md, or any forbidden files

## Open questions

None.

## Commit hashes

- `7de4c7b` — docs: design review of the reader and images (main deliverable)
- HANDOFF.md will be added in a follow-up commit

## Final check output

All checks from CLAUDE.md passed on 2026-11-14:

```
PATH=/snap/bin:$PATH cargo fmt --all
  → No output (OK)

PATH=/snap/bin:$PATH cargo clippy --workspace --all-targets -- -D warnings
  → Compiling feedrico-core v0.2.0
  → Compiling feedrico v0.2.0
  → Finished dev profile (OK)

PATH=/snap/bin:$PATH cargo test --workspace
  → 11 core tests passed
  → 155 feedrico-core tests passed
  → 4 cross-platform tests passed
  → 2 live tests ignored (needs network)
  → 1 perf test ignored (benchmark)
  → Total: 166+ tests passed (OK)

npm run check
  → Tokens built: 106 contrast pairs pass WCAG 2.2 AA
  → svelte-check: 0 errors, 0 warnings (OK)

npm test
  → Tokens built: 106 contrast pairs pass WCAG 2.2 AA
  → 37 test files, 347 tests passed (OK)
```

## PR

- **Number:** #132
- **URL:** https://github.com/viberfasend/feedrico/pull/132
- **Base branch:** main
- **Head branch:** docs/review-reader-and-images
- **Title:** docs: design review of the reader and images
- **Body:** Closes #80 — adds durable design review document
