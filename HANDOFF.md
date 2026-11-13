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
