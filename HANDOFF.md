# Handoff: site marketing copy (feat/site-marketing-copy)

## Status

Done, committed locally on `feat/site-marketing-copy` in worktree
`.claude/worktrees/vibe-site-marketing`. Not pushed (no PR, per workflow).

- `86dacd3` feat(site): speak to readers, not developers
- `7460882` feat(site): redesign the hero sun, local-only data section, full SEO pass

## What changed

The GitHub Pages site (`site/index.html`, `site/css/site.css`) was rewritten
from developer-facing to reader-facing copy:

- Hero: lede now leads with the reading experience ("reads the sites you
  choose, in the order they publish. No algorithm in the way, no account,
  nothing phoning home"); primary CTA is "See what reading looks like"
  (in-page), "Follow on GitHub" demoted to secondary; status line honestly
  says v1 is not released and will be a free download.
- New section `#why` "What Feedrico promises." with four claims: your order
  not an algorithm's, no account because there's no server, reads offline,
  fast at any size (20 or 2,000 Feeds).
- `#library` retitled "Your reading goes where you go." (was "A Library goes
  to a friend's house.").
- `#folder` "One folder, plain files." replaces the `library.json` /
  `library.db` / `.lock` file tree with plain language; keeps the
  travels/stays-on-each-machine columns, reworded. `.tree`/`.travels` CSS
  removed.
- `#privacy` ledger rows reworded for non-technical readers (GitHub Releases
  row is now just "GitHub": only when you click Check for updates).
- `#who` persona entries de-jargoned ("virtualized list" gone, OPML framed
  as "the export file from your old reader", GPL pointer removed).
- `#keys` moved below `#who` and reframed as a power-user feature.
- `#get` drops the prominent "Build it from source" box; single column with
  an honest not-released-yet note and one quiet line pointing developers at
  the README (`get__source`/`pre` CSS removed, `get__note` added).
- Nav: Why Feedrico / The window / The Library / Privacy / Who it's for /
  Get Feedrico. Meta description and og:description updated.

Copy follows `brand/BRAND.md` voice (plain, sentence case, no filler) and
`GLOSSARY.md` terms (Library, Feed, Folder, Article, Layout, View, Read,
Starred capitalised; no "database"/"sync" except to deny them). Every claim
is backed by `docs/spec/v1.md` (network policy, offline reader, image cache,
Layouts, search, performance budgets) and `docs/personas.md` (Mira, Clara,
Henrik, Jonas, Paul).

## Deviations from the brief

- Brief said "the github website"; interpreted as the GitHub Pages site in
  `site/` (deployed by `.github/workflows/pages.yml`), not the README (the
  repo page stays developer-oriented on purpose; its first lines are already
  user-facing). The repo description/topics cannot be edited with the
  available GitHub connector (no repo-settings tool), only noted.
- Kept the keyboard section and the GPL-3.0 mention (footer + one quiet line
  in #get): they are product facts, but no longer part of the pitch.

## Verification

- `npm run site:build` passes, including the WCAG 2.2 AA clock-phase
  contrast check.
- Screenshots reviewed headlessly (Chromium, 1440px light + dusk, 390px
  mobile, full page): hero, claims grid, window mock-up, story, folder,
  privacy, who, keys, get, footer all render as intended.
- Final check run:
  - `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` finished clean (50s)
  - `cargo test --workspace`: all suites ok (93 + 1 passed, rest 0/ignored)
  - `npm run check`: svelte-check 0 errors, 0 warnings
  - `npm test`: 21 files, 155 tests passed

No automated tests cover `site/` HTML/CSS; verification is the build's
contrast check plus the screenshots above.

## Second round (user follow-up): sun, folder files, SEO

- Hero sun redrawn: the filled ribbon arc became a 1.2px hairline of
  silk; the sun is a larger honey disc (r 17→20) with a softer, wider
  halo; dew drops hang below the line. Path geometry unchanged, so
  `site.js` clock positioning needed no change. Reviewed at 10:00,
  17:00 (golden) and 23:00 (lamp on the horizon), 1440px and 390px.
- `#folder` (file tree / travels-stays columns) replaced by `#local`
  "Your data stays on your machine." — one paragraph + a note that any
  backup tool or USB stick works. Rationale: users care that data is
  local, not which files are inside.
- SEO pass, informed by researching 2026 RSS-reader homepages (Feedly
  now leads with AI/CTI, Miniflux privacy-first, NetNewsWire/FreshRSS
  free+open source, FeedZero no-algorithm/no-tracking, PC Gamer's
  "kill the algorithm" RSS revival; dominant claims: no algorithm,
  privacy/no account, offline, free & open source, fast at scale):
  - `<title>`/meta/OG now say "calm RSS reader" and "free, local-first";
    keywords + robots meta added; canonical kept.
  - Hero lede and claims use the actual search terms ("desktop RSS
    reader", "chronological list", "no algorithm"); claims grid is now
    six items (added "Private by design", "Free, no subscription").
  - New `#faq` section with 6 Q&As (what is an RSS reader, offline,
    account/sync, OPML import, free & open source, tracking), mirrored as
    `FAQPage` JSON-LD together with `WebSite` + `SoftwareApplication`
    (price 0, GPL-3.0 license, featureList). The strict CSP allows the
    JSON-LD block via a sha256 hash of its exact text — recompute if the
    JSON is edited.
  - `site/robots.txt` and `site/sitemap.xml` added (Pages path
    `https://viberfasend.github.io/feedrico/`); both served correctly by
    `npm run site:preview`.

## Open questions

- When v1 ships, the hero primary CTA should become a real download button
  per OS; the "not released yet" lines in hero and #get then come out.
- If the repo description on GitHub should also change (it currently reads
  like a dev project), that needs the web UI or a token with repo-admin
  scope; the connector here has no repo-settings capability.

---

# HANDOFF — issue #77: code blocks in the reader

**Branch:** `feat/code-blocks-reader` (worktree `vibe-embeds-77`), based on `origin/main` @ `5beeb5e`.
**Status:** done, all checks green, branch pushed once, PR open (see below).

## Commits

- `2ec84af` — feat(core): keep language-* classes on pre/code through the sanitizer
- `d18d0ab` — feat(tokens): add code syntax colors with AA-checked contrast pairs
- `1237d04` — feat(ui): highlight reader code blocks locally and add a copy button
- (this file) — docs: handoff for issue #77

## What was built

1. **Sanitizer** (`crates/feedrico-core/src/sanitize.rs`): `class` is whitelisted on `pre`/`code`
   (`add_tag_attributes`) and an ammonia `attribute_filter` keeps only `language-*`/`lang-*`
   tokens (id charset-limited to `[A-Za-z0-9\-_+#.]`, ≤ 32 chars). Every other class/attribute
   stays stripped exactly as before. `plain_text`/`content_text` are untouched, so FTS is
   unaffected. No schema change → no migration, no Library version bump, version matrix A–E not
   applicable.
2. **Bundled highlighter** (no CDN, no new network surface, CSP unchanged): a small rule-based
   tokenizer compiled into the UI, living in `src/lib/components/reader/content.ts` and run by
   `enhanceContent` on the sanitized DOM per Article (`{#key article.contentHtml}`). Covers
   rust, js/ts, go, c/cpp/csharp/java/kotlin/swift, python, bash, json, yaml, toml, ini, sql,
   css, markup, diff, markdown (+ aliases); unknown languages render plain. Tokens become
   `tok-keyword/string/comment/number/function` spans.
3. **Reader styling** (`Reader.svelte`, where all `.content :global(...)` rules live):
   shaded monospace `pre` box (`color.surface.sunken`), `overflow-x: auto` + `white-space: pre`
   + `overflow-wrap: normal` inside pre so long lines scroll inside the block and never break
   the reader width; `tok-*` colors from the new `color.code.*` tokens; `.code-copy` button
   (absolute top-right) as a real `<button type="button">` — keyboard-operable, global
   `:focus-visible` ring, `aria-label` "Copy rust code block" / "Copy code block" — copies the
   `code` text only via `navigator.clipboard` with an `execCommand` fallback, shows "Copied".
4. **Tokens** (`tokens/semantic/theme.{light,dark}.json`, `tokens/contrast-pairs.json`):
   `color.code.{keyword,string,comment,number,function}` from the existing palette; each pair
   against `color.surface.sunken` (the pre background) is enforced at 4.5:1 by the token build in
   both themes (106 pairs pass).

## Personas

- **Archivist (Henrik):** dev-blog code now reads as code in batch reading; copy button is
  keyboard-reachable like everything else (`j`/`k` unaffected).
- **Accessibility spectrum:** AA contrast enforced at token build in both themes (WCAG 2.2 AA,
  4.5:1 text); copy button has an accessible name, is a native button (Tab/Enter/Space) and gets
  the visible focus ring; highlighting never changes the code text (copy/search intact).
- **Offline Reader (Paul):** highlighter is bundled; opening an Article still makes no network
  request; CSP untouched. **Skimmer:** unaffected.

## Tests added and how each was proven red

- `sanitize.rs` (red first, all 3 failed before the implementation, green after):
  `keeps_language_class_on_pre_and_code` (miniflux-style fixture: `class="language-rust"` on code,
  `lang-python` on pre), `strips_every_other_class` (Prism `token keyword` spans, class on p/span,
  mixed `language-c++ bad!`, unsafe `language-rust<script>`), `code_block_text_still_reaches_plain_text`
  (`plain_text(raw) == plain_text(sanitized)`, code text searchable → FTS unchanged).
- `reader.test.ts` "Reader code blocks" (red first: stashed `Reader.svelte`/`content.ts` changes →
  4 failed, restored → green): language class kept + `tok-*` spans + code text unchanged;
  copy button present, `<button type="button">`, aria-label, focusable (`document.activeElement`),
  click copies exactly the code text (mocked clipboard) and flips to "Copied"; bare `<pre>` is
  wrapped in `<code>` and still gets a button; `getComputedStyle` asserts `overflow-x: auto`,
  `white-space: pre`, `overflow-wrap: normal` (long lines scroll, never wrap).
- Token build: `npm run tokens` fails the build if any `color.code.*`-on-`color.surface.sunken`
  pair misses AA — 106 pairs pass in light and dark.

## Deviations from the issue

- Highlighter lives in the UI (`reader/content.ts`), not in the Rust core at parse time — the
  issue allowed either; UI-side keeps stored `content_html` small and the sanitizer diff minimal.
  After the reader redesign on main (fbc7b65), the logic was moved from `Reader.svelte` into
  `reader/content.ts` (`enhanceContent`), matching the new shape; only the `.content` CSS stays in
  `Reader.svelte` where all content styles live.
- Server-side highlighted markup (Prism/Pygments `span.token`) still renders as plain text —
  only `language-*` classes survive, per the issue.

## Open questions

- Clipboard in some Linux webviews may deny `navigator.clipboard`; the `execCommand` fallback
  covers it, but it is untested on a real WebKitGTK build.
- The highlighter is intentionally small (~15 languages); extending it is a data-only change in
  `LANGS`.

## Final check output (PATH=/snap/bin:$PATH, CARGO_TARGET_DIR=…/shared-target)

```
=== cargo fmt --all ===
(clean)
=== cargo clippy --workspace --all-targets -- -D warnings ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.10s
=== cargo test --workspace ===
test result: ok. 4 passed; 0 failed; …
test result: ok. 102 passed; 0 failed; …
test result: ok. 1 passed; 0 failed; …
(… all suites ok)
=== npm run check ===
svelte-check found 0 errors and 0 warnings
=== npm test ===
 Test Files 22 passed (22)
      Tests 200 passed (200)
=== npm run tokens (AA) ===
Tokens built; 106 contrast pairs pass WCAG 2.2 AA in light and dark.
```
