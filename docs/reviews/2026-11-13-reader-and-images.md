# Design Review: Reader and Images

**Date:** 2026-11-13  
**Issue:** [#80](https://github.com/viberfasend/feedrico/issues/80)  
**Status:** Design completed; implementation landed on `main`  
**Author:** andi1984 (Andreas Sander)

This document captures the design review of the reader component (List pane and grid slide-over), focused on how images are presented and handled. The review was performed on branch `design/reader-drawer` and the implementation has since been merged to `main`. For the current specification, see the [Reader](#reader) section in [`docs/spec/v1.md`](../../spec/v1.md#reader).

---

## Method

The review followed a two-phase approach:

1. **Research:** surveyed how established feed readers (NetNewsWire, Feedbin, Miniflux, Reeder, Unread, NewsFlash, News Explorer) present Articles and handle images.
2. **Mock rendering:** rendered the current `main` branch with a scenario mock (`.claude/skills/debugging-feedrico/mock-reader.js`) containing 11 real-world Articles, tested in both **Cards** and **List** Layouts at the 880 px minimum window width, in both light and dark themes.

---

## Findings on `main` (pre-redesign)

The mock render surfaced the following issues across common Article types:

| Scenario | Issue on `main` |
|---|---|
| WordPress post | 72 px emoji images break paragraphs; share icons stack vertically; the 1×1 tracking pixel takes a line |
| Photo set | Fine overall, but portrait photos run past the screen; no way to see a picture larger |
| Webcomic | The title text (the punchline) is a hover-only tooltip; a 1800×520 strip shrinks to 151 px tall |
| Summary-only news | The card shows a Lead Image, but the reader shows none and ends without a "read the rest" link |
| Podcast | Raw file names and MIME types displayed; **clicking an attachment does nothing** (bug); cover art not shown |
| Technical post | Black-on-transparent diagram invisible in the dark theme; 16 px icon breaks the sentence |
| Offline images | Dashed card with a long repeated label |
| Long title, long Feed name | Feed name wraps into the author line; German compounds can widen the column |
| Title only | Empty page |
| Chrome | Actions scroll away with the header; close button above the Feed name; no prev/next |

---

## What Other Readers Do

Established readers handle images and related content in the following ways:

- **NetNewsWire** (open source): lead image is only shown when the body doesn't contain it (URL match plus WordPress `-WxH` resize suffix); `img.wp-smiley` replaced by its alt text; applies `height: auto !important` to images; no breakout of images beyond their natural size, no height cap; enclosures are displayed after the body.
- **Feedbin** (open source): hides `img[width=1][height=1]` (tracking pixels); breaks images out to full width only when `naturalWidth >= column`; neutralises floats; uses click-to-load placeholders for iframes.
- **Miniflux:** uses plain `max-width: 100%` on images, renders figcaptions in uppercase, has no lightbox/viewer.
- **Reeder:** includes an image viewer; linked images open in the viewer with the link shown as a button; as of 2025.3, article width is adjustable; as of 2025.4.1, "Articles without images now show the thumbnail".
- **Unread, NewsFlash, News Explorer:** each provides a dedicated image viewer.

Notably, **no surveyed reader shows comic title text inline or caps tall portrait photos** to the view height; both decisions became Feedrico-specific calls.

---

## Decisions

Each decision is a simple, testable rule. The implementation now reflects these in the [Reader](#reader) section of [`docs/spec/v1.md`](../../spec/v1.md#reader).

### Image Treatment

Images are treated **by size alone**, avoiding heuristics based on URLs or class names:

- **Hidden:** images ≤ 2 px (tracking pixels, spacers) are hidden.
- **Inline:** images ≤ 32 px (icons, share buttons) are kept inline within text.
- **Emoji alt:** when an image's alt text is an emoji, the emoji text is shown instead of the image.
- **Block:** larger images are rendered as centred blocks:
  - Capped to the text column width and the pane height, so a whole photo fits on screen.
  - Images more than twice as tall as wide (comics, infographics) are **not** capped and scroll vertically.
  - Images at least twice as wide as tall and wider than the column (strips, panoramas) may use the pane width, never more than their intrinsic size.

### Lead Image

The **Lead Image** leads the Article in the reader **only when the content has no picture of its own**. This rule:

- Avoids NetNewsWire's URL-based heuristics (which can produce false matches).
- Ensures a picture is never shown twice (once as Lead Image and once in content).
- For summary-only feeds (no content images), photo enclosures, and podcast enclosures, the Lead Image from the card/magazine is carried into the reader.

### Image Viewer

- Clicking a picture (or pressing `Enter` on it) opens it full-window, fitted to the viewport.
- `z` or a click toggles between fitted and real (1:1) size.
- `←` / `→` navigates between the Article's pictures.
- `Esc` closes **only the viewer** — if a viewer is open over the reader, `Esc` does not close the reader.
- Pictures that are inside a link open the link instead of the viewer.
- The viewer loads nothing remote; it presents the cached image with explicit "original" and "link" buttons.

### Other Content

- **No inline audio/video player in v1** (per [spec Out of scope](../../spec/v1.md#out-of-scope-for-v1)).
- **No image grids or carousels** in v1: stacked presentation plus the viewer is simpler and avoids cropping photos. A future version may revisit grids.

---

## Follow-up Issues

The review spawned the following issues, all filed from this design work:

### Implemented (on `design/reader-drawer`)

- [#81](https://github.com/viberfasend/feedrico/issues/81) — Images by size (P1)
- [#82](https://github.com/viberfasend/feedrico/issues/82) — Image viewer (P2)
- [#83](https://github.com/viberfasend/feedrico/issues/83) — Lead Image leads summary-only Articles (P1)
- [#84](https://github.com/viberfasend/feedrico/issues/84) — Attachments do nothing when clicked + readable cards (P1, bug)
- [#85](https://github.com/viberfasend/feedrico/issues/85) — Reader toolbar, header and end of the Article (P1)

### Deferred

- [#86](https://github.com/viberfasend/feedrico/issues/86) — Don't download tracking pixels at fetch (P2, core)
- [#87](https://github.com/viberfasend/feedrico/issues/87) — "Video on youtube.com" embed cards (P3, core)
- [#88](https://github.com/viberfasend/feedrico/issues/88) — List Layout at 880 px leaves the reader 225 px (P2)
- [#89](https://github.com/viberfasend/feedrico/issues/89) — Later: resizable slide-over (P3)

---

## Personas Impacted

This review primarily benefits:

- **The Skimmer ([Clara](../../personas.md#the-skimmer))**: improved photo, comic and news feed presentation in Cards, Magazine and Gallery; better Lead Image handling for image-heavy Feeds.
- **The Archivist ([Henrik](../../personas.md#the-archivist))**: WordPress-heavy Feeds now render cleanly as text; keyboard operation for the viewer (navigate with `←`/`→`, close with `Esc`); tooltips on toolbar buttons show shortcuts.
- **The Offline Reader ([Paul](../../personas.md#the-offline-reader))**: tracking pixels hidden and not fetched at read time; explicit network buttons (open in browser, original/link in viewer).
- **The Newcomer ([Mira](../../personas.md#the-newcomer))**: no empty or broken-looking Articles (title-only Articles now show a message; summary-only Articles show their Lead Image in the reader).

Accessibility spectrum benefits:

- Emoji images replaced by their alt text (screen reader friendly).
- Image captions (from `title` attributes) shown explicitly.
- Black-on-transparent diagrams placed on a light backdrop in dark theme for readability.
- Full keyboard operation for the viewer.

The Library format, locking and startup are **not** touched, so the [Carrier](../../personas.md#the-carrier) and [Host](../../personas.md#the-host) personas are unaffected.

---

## Sources

Primary sources consulted during the research phase:

- **NetNewsWire:** [`Shared/Article Rendering/`](https://github.com/Ranchero-Software/NetNewsWire/tree/main/Shared/Article%20Rendering/) in [Ranchero-Software/NetNewsWire](https://github.com/Ranchero-Software/NetNewsWire) — specifically `stylesheet.css`, `main.js`, `ArticleRenderer+PrimaryMedia.swift`, `+Enclosures.swift`
- **Feedbin:** [`app/assets/stylesheets/application.scss`](https://github.com/feedbin/feedbin/blob/master/app/assets/stylesheets/application.scss), [`app/assets/javascripts/web/_site.js.coffee`](https://github.com/feedbin/feedbin/blob/master/app/assets/javascripts/web/_site.js.coffee) in [feedbin/feedbin](https://github.com/feedbin/feedbin)
- **Miniflux:** [`internal/ui/static/css/common.css`](https://github.com/miniflux/v2/blob/master/internal/ui/static/css/common.css) in [miniflux/v2](https://github.com/miniflux/v2)
- **Reeder:** [App Store page](https://apps.apple.com/app/id6475002485) — release notes for 2025.3 and 2025.4.1
- **Unread:** [MacStories review of Unread 4.0](https://www.macstories.net/reviews/unread-4-0-brings-a-fast-and-native-rss-reader-to-the-mac/) — mentions the image viewer
- **NewsFlash:** [Developer blog](https://blogs.gnome.org/jangernert/) — image viewer notes

---

## See Also

- [Reader specification](../../spec/v1.md#reader) — the implemented design in the v1 spec
- [Content safety and privacy](../../spec/v1.md#content-safety-and-privacy) — image caching, sanitization and network policy
- [Image cache specification](../../spec/v1.md#image-cache) — caching rules that inform image behaviour
- [Personas](../../personas.md) — the user roles this design serves
