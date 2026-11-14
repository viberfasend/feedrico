# HANDOFF: Issue #87 — Name video embeds: "Video on youtube.com: open in browser"

## Status
✅ **COMPLETE** — PR #130 created and pushed to origin/feat/core-video-embed-cards

## Issue
- **Number:** #87
- **Title:** Name video embeds: "Video on youtube.com: open in browser"
- **Area:** content, core sanitizer

## What was built

Modified `crates/feedrico-core/src/sanitize.rs` to recognize and name known video hosting platforms in embed cards:

### Changes to `VideoHost` enum
- Added `Dailymotion` variant
- Added `PeerTube` variant
- Updated `as_str()` and `label()` methods for both

### Changes to `video_identity()` function
- Added Dailymotion detection: matches `dailymotion.com` and `www.dailymotion.com` with `/embed/video/<id>` path
- Added PeerTube detection: matches any host with `/videos/embed/<id>` path pattern
- Updated function documentation

### Changes to `embed_card` closure in `sanitize()`
- Introduced `display_host` variable to handle platform-specific display logic
- For PeerTube: uses the actual hostname from the URL (e.g., "peertube.example.com")
- For YouTube/Vimeo/Dailymotion: uses the canonical domain (youtube.com, vimeo.com, dailymotion.com)
- This ensures "Video on <host>" shows the correct host for each platform

### New tests added (test-first approach)
1. `peertube_embeds_become_video_cards` — verifies PeerTube embeds produce "Video on <instance-host>" cards
2. `dailymotion_embeds_become_video_cards` — verifies Dailymotion embeds produce "Video on dailymotion.com" cards
3. `youtube_nocookie_embeds_become_video_cards` — verifies youtube-nocookie embeds produce "Video on youtube.com" cards

## Test-first verification

All three new tests were added and verified to **FAIL** before implementation:
- PeerTube test: Expected "Video on peertube.example.com" but got "Embedded content on peertube.example.com"
- Dailymotion test: Expected "Video on dailymotion.com" but got "Embedded content on dailymotion.com"
- YouTube nocookie test: **PASSED** (already worked due to existing youtube-nocookie.com handling)

After implementation, all three tests pass.

## Final check output

All checks from CLAUDE.md passed:

```
✅ PATH=/snap/bin:$PATH cargo fmt --all
✅ PATH=/snap/bin:$PATH cargo clippy -p feedrico-core -- -D warnings
✅ PATH=/snap/bin:$PATH cargo test -p feedrico-core (158 tests passed)
✅ npm run check (tokens built, svelte-check: 0 errors)
✅ npm test (347 tests passed)
```

## Commit

- **Hash:** cec73cd
- **Message:** `feat(core): name known video hosts in embed cards`
- **Files changed:** 1 file (`crates/feedrico-core/src/sanitize.rs`)
- **Lines changed:** +104, -4

## PR

- **Number:** #130
- **URL:** https://github.com/viberfasend/feedrico/pull/130
- **Base branch:** main
- **Head branch:** feat/core-video-embed-cards

## Deviations from brief

None. All requirements met:
- ✅ Only edited `crates/feedrico-core/src/sanitize.rs`
- ✅ Modified the `embed_card` closure inside `sanitize()` (around line 177)
- ✅ Added unit tests in the existing `#[cfg(test)]` module
- ✅ Test-first approach used and verified
- ✅ Did not touch img/tracking-pixel handling
- ✅ No drive-by formatting
- ✅ No changes to forbidden files

## Open questions

None.
