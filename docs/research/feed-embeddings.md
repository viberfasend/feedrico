# Research: embeddings in Feeds (YouTube videos, LaTeX/math, code snippets)

Status: research for issue filing. 2026-11. Worktree `vibe-embeds-research`, branch
`research/feed-embeds`. Vocabulary follows `GLOSSARY.md`; personas from
`docs/personas.md`.

## Method

- Read the current pipeline end to end: `crates/feedrico-core/src/parse.rs` (feed-rs 3.0.0),
  `src/sanitize.rs`, `src/schema.rs`, `src/articles.rs`, `src/lib/components/Reader.svelte`,
  and `docs/spec/v1.md` ("Content safety and privacy", "Articles", "Out of scope for v1").
- Read the feed-rs 3.0.0 source (`~/.cargo/registry/src/.../feed-rs-3.0.0/`) to know exactly
  which feed fields survive parsing: `model/mod.rs`, `parser/mediarss.rs`, `parser/rss2/`,
  `parser/atom/`, `parser/json/`, plus the bundled YouTube fixture
  `fixture/atom/atom_mediarss_youtube_1.xml`.
- Web research on what real feeds carry: Media RSS spec (rssboard.org), YouTube channel feed
  format, JSON Feed 1.1, blog `content:encoded` embeds (iframes, script embeds, gists),
  code-block conventions, math-in-RSS practice (MathML, KaTeX/MathJax, codecogs images, arXiv).

## What feeds actually give us

### 1. RSS 2.0 core

- `<enclosure url length type>` — one attachment per item; podcasts (audio/mpeg), video files
  (video/mp4). feed-rs wraps it in a `MediaObject` (`MediaObjectSource::RSS`).
- `<description>` (summary) vs `content:encoded` (full HTML). feed-rs puts `content:encoded`
  into `entry.content.body`, so full HTML is available today.

### 2. Media RSS (`http://search.yahoo.com/mrss/`, rssboard spec)

Per item, flat or inside `<media:group>`:

- `<media:content url type medium isDefault width height duration fileSize bitrate>` — multiple
  allowed. feed-rs 3.0 keeps `url, content_type, width, height, duration, size, rating` in
  `MediaContent`; it **drops `medium`, `isDefault`, `bitrate`** (no such fields).
- `<media:player url>` — feed-rs folds the player URL into `content.url` when `media:content`
  has no URL (`parser/mediarss.rs:243`).
- `<media:thumbnail url width height>` → `MediaThumbnail` (used today for Lead Image).
- `<media:title>`, `<media:description>`, `<media:community>` → `MediaCommunity { stars_avg,
  stars_count, stats_views, stats_favorites }`; `media:text` (captions/transcripts),
  `media:credit`, podcast `transcripts`, `season`, `episode`, `people`.
- feed-rs 3.0 has **no `extensions` map anymore**: unknown namespaces are dropped entirely.

### 3. YouTube channel feeds (the "video Feed" case)

`https://www.youtube.com/feeds/videos.xml?channel_id=UC…` serves Atom with
`xmlns:yt="http://www.youtube.com/xml/schemas/2015"`. Per entry (see feed-rs's own fixture):

- `<id>yt:video:VIDEO_ID</id>`, `<yt:videoId>`, `<yt:channelId>` — **dropped by feed-rs 3.0**;
  the video ID is recoverable from the entry id prefix `yt:video:` or the alternate link
  `https://www.youtube.com/watch?v=VIDEO_ID`.
- `<media:group>` with `media:title`, `media:thumbnail`
  (`https://i.ytimg.com/vi/ID/hqdefault.jpg`, 480x360), `media:description`,
  `media:community` (`starRating count/average`, `statistics views favorites`), and a
  `media:content url="https://www.youtube.com/v/ID?version=3"
  type="application/x-shockwave-flash"` — a **dead Flash-era URL**.
- Today that Flash URL passes the `application/` filter in `parse.rs`, so YouTube Articles get
  a useless enclosure link. The thumbnail already works as Lead Image.

### 4. JSON Feed 1.1

- `items[].content_html` (full HTML, same as `content:encoded`), `attachments[]` with
  `url, mime_type, size_in_bytes, duration_in_seconds`, `banner_image`.
- feed-rs maps `attachments[]` to `entry.links` (with `media_type`/`length`), **not** to
  `entry.media` — so today's `parse.rs` (which reads only `entry.media` for enclosures) misses
  JSON Feed attachments.

### 5. Embeds inside article HTML (`content:encoded`)

Blogs port their web embeds into the feed: `<iframe src="https://www.youtube.com/embed/ID">`
(also Vimeo, SoundCloud, Spotify), `<video><source …>`, `<embed>`, `<object>`, and script
embeds (`<script src="https://gist.github.com/…">`, X/Twitter `<blockquote class="twitter-tweet">`
+ widget script). Today `sanitize.rs` replaces **all** of `iframe, embed, object, video, audio`
with one generic link card ("Embedded content on youtube.com: open in browser") — no
thumbnail, no video ID, no distinction between playable media and social posts.

### 6. Code snippets

No standard. De facto shapes in `content:encoded`:

- `<pre><code class="language-rust">…</code></pre>` (CommonMark/GFM convention),
- server-side highlighted markup: `<span class="token keyword">` etc. (Prism, Highlight.js,
  Pygments) whose colors come from a CSS file readers don't have,
- GitHub release/commit feeds carry diffs as `<pre>`-style HTML.

Today ammonia keeps the `pre`/`code` tags but strips **all classes** (only `p.embed-link` is
allowed), so the language is lost; `Reader.svelte:305-316` already styles `pre`/`code`
monospace. Miniflux has the same gap filed as an open feature request (miniflux/v2#4587: keep
`language-*` classes).

### 7. LaTeX / math

No RSS standard at all. Real-world shapes:

- raw TeX delimiters in the text: `\(...\)`, `\[…\]`, `$…$`, `$$…$$` (often XML-escaped);
- pre-baked **MathML** `<math>…</math>` (W3C; natively rendered by modern browser engines and
  the best screen-reader support — MathML is *the* accessible-math format);
- KaTeX/MathJax server-rendered HTML (`<span class="katex">` + inline styles) — inline styles
  and classes are stripped by our sanitizer, so it degrades to plain text;
- equation images via services like `latex.codecogs.com/png.latex?…` — these arrive as normal
  `<img>` and already work through the media cache;
- arXiv's Atom feed carries only title/abstract/authors/links — no full text, no TeX source.

Today ammonia's allow-list drops `<math>` entirely, so MathML equations **vanish** from
Articles; raw `$…$` survives as literal text.

## Gaps in Feedrico today (mapped to code)

| Feed signal | Today | Gap |
|---|---|---|
| `media:thumbnail` (YouTube `hqdefault.jpg`) | Lead Image + cached | works |
| `yt:video:` id / `watch?v=` link | ignored | no video identity, no rich card |
| `media:community` views/stars | dropped by feed-rs field set? kept in model, unused by us | no metadata on video cards |
| `media:content` flash URL (`application/x-shockwave-flash`) | becomes an "enclosure" | dead link shown to user |
| `media:content` duration / `media:player` / `medium` | duration kept in model, unused; player folded into url; medium dropped | enclosures show bare filename |
| JSON Feed `attachments[]` | land in `links`, not read as enclosures | missing |
| `<iframe>`/`<video>` in content | generic link card | no thumbnail/metadata card |
| `<pre><code class="language-x">` | tags kept, class stripped | no highlighting, no copy button |
| `<math>` MathML | stripped by ammonia | equations disappear |
| raw `$…$` / `$$…$$` | literal text | unreadable math |
| codecogs equation `<img>` | cached and shown | works |

Spec constraints to respect (`docs/spec/v1.md`): iframes are replaced by link cards, "No inline
player in v1", reading an Article must make **no network request** (Offline Reader, Paul), no
CDN/scripts (bundled assets only), strict CSP. So the right shape is *richer local cards*
(cached thumbnail, metadata, open-in-browser), not live iframes.

## Proposed issue breakdown

1. Video embeds & video Feeds: detect YouTube/video iframes and `yt:` feeds; rich card with
   cached thumbnail, duration, view count; fix the Flash-URL enclosure. Library format change
   (video metadata on Article) → migration + version matrix A–E. area:content + area:ui, P2.
2. Enclosure/attachment completeness: duration + player + `medium`/`isDefault` from MediaRSS,
   JSON Feed `attachments[]` → enclosures, drop dead Flash URLs. area:content, P3.
3. Code blocks in the reader: keep `language-*` classes through the sanitizer, local syntax
   highlighting (bundled, no CDN), copy button, horizontal scroll. area:content + area:ui, P2.
4. Math in Articles: pass MathML through the sanitizer + reader styling (accessibility win),
   decide on rendering raw TeX delimiters locally (bundled KaTeX, no CDN) or preserving them.
   area:content + area:ui + accessibility, P3.

## Sources

- feed-rs 3.0.0 source + fixtures (`~/.cargo/registry/src/…/feed-rs-3.0.0/`), incl.
  `fixture/atom/atom_mediarss_youtube_1.xml`
- [Media RSS Specification](https://www.rssboard.org/media-rss),
  [simplepie-ng Media RSS spec notes](https://github.com/simplepie/simplepie-ng/wiki/Spec:-Media-RSS)
- [JSON Feed 1.1](https://www.jsonfeed.org/version/1.1/)
- YouTube channel feed format: `youtube.com/feeds/videos.xml?channel_id=…` (Atom + `yt:` ns),
  e.g. Stack Overflow "XSLT transform of Youtube Channel feed"
- Math in RSS: noamross.net "Displaying Math in RSS feeds" (codecogs images), Tobias Franke
  "Pre-baked MathML in Jekyll", giannipetrella.eu (MathML-only feeds), U. Washington/Penn
  State MathML accessibility guides, MathJax a11y docs
- Code in feeds: miniflux/v2#4587 (keep `language-*`), westonruter/syntax-highlighting-code-block#714,
  CSS-Tricks "Posting Code Blocks on a WordPress Site", andrewbaker.ninja (server-rendered
  highlighting keeps code intact in readers)
- Embeds in feeds: roboleary.net "Third-party embeds" (readers strip iframes/scripts),
  jonathanfrei.com "Easy URL embeds" (YouTube/X/Vimeo/Spotify embed markup)
- arXiv help: RSS/Atom feeds carry metadata + abstract only
