# Feedrico brand guidelines

For anyone designing or building something that carries the Feedrico name: the website first,
then release pages, docs and the app's own welcome screen. Vocabulary follows
[`GLOSSARY.md`](../GLOSSARY.md): Library, Feed, Folder, Article, Layout, View, Starred, Read.

## The idea

Feedrico is named after a friend: "Feed" plus Federico. Its look comes from the
**Altweibersommer**, the warm weeks at the end of summer when the light turns golden, the
afternoons get long and quiet, and fine threads of spider silk drift through the air and catch
the sun.

That is the feeling we want reading to have: late light, no hurry, nothing pulling at you. No
algorithm and no account. Everything lives in one **Library** folder that belongs to you, so
you can put it on a USB stick and open it on a friend's machine, with every Article where you
left it.

**The mark is the feed symbol, spun from silk:** a drop of honey light where the feed dot sits,
and two threads of gossamer where the waves would be. It tells you what the product does and
what it feels like in one small drawing.

## Logo

### The parts

| Part | What it is |
|---|---|
| **Mark** | The honey drop and two tapered silk threads. The threads swell and thin like a pen stroke, which echoes Literata's own stroke contrast. |
| **Wordmark** | "Feedrico" in Literata (optical size 60, weight 540), outlined to paths. Two drawn details: the tittle of the *i* is the same honey drop, and a single strand of silk hangs from the top of the *d* to it. |
| **Lockups** | Horizontal (mark left of the wordmark) and stacked (mark above). Never rebuild them by hand: use the files. |

The mark comes in three **optical weights** that share one geometry, so it reads at every size
without changing shape:

- **fine**: thin threads with dew and loose ends. Only for the app icon at 64 px and up and
  for large artwork such as the social card.
- **logo**: medium threads that match the wordmark stems. Used in every `logo/` file.
- **small**: heavy threads, no dew. Used for favicons and icons at 48 px and below.

### Clear space and minimum sizes

- **Clear space:** keep at least **one drop** (the diameter of the honey drop at the size you
  use) free on every side of the mark, the wordmark or a lockup. Nothing else goes there.
- **Minimum sizes:**
  - mark: 24 px. Below that, use `favicon/favicon.svg` (small weight).
  - horizontal lockup: 24 px tall. The silk strand disappears below about 48 px tall. That is
    intended: the drop stays.
  - wordmark: 96 px wide.
  - stacked lockup: 72 px tall.

### Colour versions

| Version | Use on | Threads and letters | Drop |
|---|---|---|---|
| default | paper and light surfaces (`--color-surface-*` in light) | dusk ink `#2B2541` | honey `#E89C26` |
| `-reversed` | dusk and dark surfaces | paper `#FBF7EF` | warm honey `#F3B84B` |
| `-mono` | anywhere a single colour is needed: print, engraving, photos, mid-tone backgrounds | `currentColor` | `currentColor` |

`-mono` files use `currentColor`. Inline them (or use them as a CSS mask) and set `color`. As
an `<img>` they render black.

### Do

- Use the files as they are, scaled proportionally.
- Use the reversed version on dusk and the default version on paper.
- Pair the logo with plenty of empty space. It is a quiet logo; let it be quiet.

### Don't

- Don't recolour the drop to anything but honey (or `currentColor` in mono).
- Don't add gradients, shadows, glows, outlines or bevels to the logo. The glow belongs to the
  app icon and the social card only.
- Don't rotate, mirror, stretch or re-space the mark or the wordmark.
- Don't set "Feedrico" in live type as a logo. In running text, write Feedrico in the
  surrounding font.
- Don't use the mark as a generic "subscribe via RSS" button. It is our name, not the standard
  feed icon.
- Don't draw spiders, orb webs or anything Halloween. Our silk drifts in the sun; it does not
  catch things.
- Don't put the colour logo on photos or busy backgrounds. Use mono.

## Asset index

Everything here is generated from one geometry source (see [Rebuilding](#rebuilding)). All
SVGs are plain paths with no embedded rasters and no font dependencies.

### `logo/`

| File | Purpose |
|---|---|
| `feedrico-mark.svg` | Mark, default colours, for light backgrounds |
| `feedrico-mark-reversed.svg` | Mark for dark backgrounds |
| `feedrico-mark-mono.svg` | Mark in `currentColor` |
| `feedrico-wordmark.svg` / `-reversed` / `-mono` | Wordmark alone, for where the mark already appears nearby (for example, the site footer next to the icon) |
| `feedrico-lockup-horizontal.svg` / `-reversed` / `-mono` | The default logo: site header, README, release notes |
| `feedrico-lockup-stacked.svg` / `-reversed` / `-mono` | Centred layouts: welcome screen, about box, the end of a long page |

### App icon

| File | Purpose |
|---|---|
| `app-icon.svg` | **Master** app icon, 1024 px on the macOS grid (824 px tile, ~185 px corner radius, soft shadow in the margin). Dusk tile, honey drop, fine threads with dew. Reads on light and dark docks thanks to a faint paper edge. |
| `app-icon-small.svg` | Same icon for 16–48 px: near full-bleed tile and small-weight threads |
| `app-icon-touch.svg` | Full-bleed opaque square for `apple-touch-icon` and maskable web icons |

`src-tauri/icons/` is generated from these by `brand/tools/export.sh`.

### `favicon/`

| File | Purpose |
|---|---|
| `favicon.svg` | Scalable favicon. Mark only (small weight), no tile; switches ink and drop colour with `prefers-color-scheme` |
| `favicon.ico` | 16, 32 and 48 px fallback for old browsers |
| `favicon-16.png`, `favicon-32.png` | PNG fallbacks |
| `apple-touch-icon.png` | 180 px, full-bleed |
| `icon-192.png`, `icon-512.png` | Web app manifest icons (`purpose: any`) |
| `icon-maskable-512.png` | Web app manifest icon (`purpose: maskable`) |

```html
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="icon" href="/favicon.ico" sizes="48x48">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<meta name="theme-color" content="#FBF7EF" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#1E1A2F" media="(prefers-color-scheme: dark)">
<meta property="og:image" content="https://<site>/og-card.png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="Feedrico: a calm reader for the Feeds you follow.">
<meta name="twitter:card" content="summary_large_image">
```

### `social/`

| File | Purpose |
|---|---|
| `og-card.svg` | 1200×630 source: the fine mark at dusk, oversized and cropped at the right edge, beside the headline. All text outlined. |
| `og-card.png` | The export to use in `og:image` |

### `motif/`

| File | Purpose |
|---|---|
| `gossamer-thread.svg` | One thread sagging between two points, with dew. Under a heading, as a section divider |
| `gossamer-drifting.svg` | Three threads crossing on warm air, 1200×240. Hero backgrounds, wide bands, page ends |
| `gossamer-corner.svg` | A thread anchored in a corner. Card and panel corners; mirror it with `transform: scale(-1, 1)` for other corners |
| `gossamer-dew.svg` | A short strand with three dew drops. Small accent: list ends, footnotes, "all caught up" |

In the app, the same drawing lives in `src/lib/components/Gossamer.svelte`.

## Colour

The palette is the token palette: no separate "brand colours". Primitives live in
`tokens/primitive/color.json`; UI and website code use the **semantic** CSS custom properties
built by `npm run tokens` into `src/lib/tokens/generated/` (`:root` / `[data-theme="light"]`
and `[data-theme="dark"]`). Copy those files into the site rather than retyping values.

### The five colours that make Feedrico

| Name | Hex | Primitive | Role |
|---|---|---|---|
| Paper | `#FBF7EF` | `palette.paper.50` | Light background, reversed logo |
| Honey | `#E89C26` | `palette.amber.400` | The drop; accent fills in light |
| Warm honey | `#F3B84B` | `palette.amber.300` | The drop on dusk; accent in dark |
| Dusk ink | `#2B2541` | `palette.dusk.800` | Logo ink; raised surfaces in dark |
| Dusk | `#1E1A2F` | `palette.dusk.900` | Dark background, social card |

Supporting: the full `paper`, `amber` and `dusk` ramps, plus `moss` (success) and `rust`
(danger) for status only.

### Semantic tokens to use

| Role | Token | Light | Dark |
|---|---|---|---|
| Page background | `--color-surface-base` | paper 50 | dusk 900 |
| Cards, panels | `--color-surface-raised` | paper 0 | dusk 800 |
| Wells, code, sidebars | `--color-surface-sunken` | paper 100 | dusk 950 |
| Body text | `--color-text-default` | paper 900 | paper 100 |
| Secondary text | `--color-text-muted` | paper 600 | dusk 200 |
| Accent text and links | `--color-text-accent`, `--color-text-link` | amber 700 | amber 300 |
| Primary button fill | `--color-accent-default` (hover `--color-accent-hover`) | amber 400 | amber 300 |
| Text on a button | `--color-text-on-accent` | paper 950 | paper 950 |
| Quiet highlight | `--color-accent-subtle` | amber 100 | dusk 700 |
| Focus ring | `--color-border-focus` | amber 600 | amber 300 |
| Hairlines | `--color-border-subtle` / `-default` | paper 200 / 300 | dusk 800 / 700 |
| Gossamer threads | `--color-motif-gossamer` | paper 300 | dusk 600 |
| Dew, unread dot | `--color-unread-indicator` | amber 600 | amber 300 |

Rules:

- **Honey is light, not ink.** Use it as a fill (buttons, the drop, dew, the unread dot) and
  put `--color-text-on-accent` on top. Text in the accent colour uses `--color-text-accent`
  (amber 700 in light), never honey on paper: it fails contrast.
- **One point of light per view.** Honey is precious. A screen with honey everywhere is a
  screen with no light in it.
- Every text/background pair the token build knows is checked against WCAG 2.2 AA. If the site
  adds new pairs, check them too.

## Typography

Both families are open-licensed (SIL OFL) and bundled through `@fontsource-variable`. Self-host
them; never load fonts from a CDN (Feedrico makes no third-party requests, and its website
shouldn't either).

| Family | Token | Role |
|---|---|---|
| **Literata** (variable: `opsz` 7–72, `wght` 200–900) | `--font-family-reading` | Headlines, Article text, pull quotes, anything meant to be *read*. The wordmark. |
| **Inter** (variable: `opsz` 14–32, `wght` 100–900) | `--font-family-ui` | Buttons, navigation, labels, metadata, captions, tables: anything meant to be *used*. |

- Keep `font-optical-sizing: auto` so Literata tightens for display sizes and opens up for small
  text.
- Headlines: Literata at weight 480–560, not bold. Our voice is calm; heavy headlines shout.
  The app's `--font-weight-heading` (700) is for small UI titles only.
- Body on the site: Literata 18–20 px, line height `--font-line-height-reading` (1.65), measure
  60–70 characters. UI text: Inter at `--font-size-body`, line height 1.35–1.5.
- App scale (tokens): caption 12 · small 13 · body 14 · title-sm 16 · reading 18 · title-md 20 ·
  title-lg 24 · title-display 38 px. For the website, continue it upward on a 1.25 ratio: 48,
  60, 75 px for hero headlines.
- **Numerals:** Inter with `font-variant-numeric: tabular-nums` for counts, dates and anything
  in columns (the app does this). Literata with `oldstyle-nums` in running text, so numbers sit
  quietly inside a sentence.
- Sentence case everywhere: headings, buttons, menus. No all-caps labels, no letter-spaced
  eyebrows above headings.
- Italic Literata is for titles of works and gentle emphasis, never for decoration.

## The motif

The gossamer threads are the brand's texture. Use them like the real thing: a few strands,
noticed only when the light falls right.

- **Colour:** threads in `--color-motif-gossamer`, dew in `--color-unread-indicator`. The motif
  SVGs already read these variables (with light-theme fallbacks), so when inlined they follow
  the theme by themselves.
- **Where:** empty states, the welcome screen, the hero, section transitions, the footer. One
  motif per view at most.
- **Never** behind body text, as a repeating pattern, at full opacity in large areas, or
  coloured honey (honey is for the dew).
- Threads drift **left to right and slightly upward**, as if lifted by warm air. Keep that
  direction when you mirror or crop.
- Drawing your own: tapered strokes (thick in the middle, needle-thin at the ends), one loose
  end that curls off, dew drops that *hang below* the thread, never sit on top of it.
  `brand/tools/geometry.py` has `ribbon()` for this.

## Imagery and illustration

- **Illustration** is line drawing in the thread language: tapered strokes and hairlines in
  ink or paper, with honey for the single point of light (a lamp, the sun, a drop, an unread
  dot). Flat, no 3D, no gradients except the soft halo around a light.
- **Subjects:** the real things around reading. A desk by a window in late light, a USB stick
  with a paper tag, a train window, a friend's kitchen table. Objects, not people's faces.
- **Photography**, if any: natural late-afternoon light, warm and slightly underexposed, real
  places. No stock shots of smiling people with laptops, no neon, no abstract 3D blobs.
- **Product screenshots** are the best imagery we have. Show the app in both themes, at real
  size, with the reading typography visible.

### Mock-up content: placeholder only

**No brand asset, screenshot or mock-up may contain real RSS feed data.** Any picture of the
app (website, social cards, illustrations, docs, examples in this file) uses invented
placeholder content only:

- Feed names that are clearly fictional: "Lorem Ipsum Weekly", "The Dolor Review",
  "Consectetur Notes", "Amet Field Journal".
- Article titles and excerpts in lorem ipsum: "Dolor sit amet, consectetur adipiscing",
  "Sed do eiusmod tempor incididunt".
- No real blogs, sites, people, brands, logos, favicons or headlines. Feed icons in mock-ups
  are plain letter tiles in palette colours.
- Lead Images in mock-ups are our own illustrations or abstract colour fields, never pictures
  taken from real Feeds.

Build screenshots from a demo Library filled with this placeholder content.

## Motion

Motion in Feedrico is the pace of a late-summer afternoon.

- **Calm:** use the motion tokens (`--motion-duration-fast` 140 ms to `--motion-duration-slow`
  320 ms with `--motion-easing-standard` / `-enter` / `-exit`) for anything that answers a
  click or keypress. Nothing bounces, nothing overshoots.
- **One orchestrated moment per page**, at most. On the site that could be the threads drifting
  in the hero: very slow (20 s or more per cycle), a few pixels of travel, never looping
  visibly.
- Motion shows what changed (a panel opening, an Article marked Read), never decorates.
- **`prefers-reduced-motion: reduce`** turns off all ambient drift, parallax and scroll-linked
  animation, and replaces transitions with instant changes or short cross-fades. The page must
  be complete without any motion.

## Voice and tone

Feedrico talks like a well-read friend who respects your time: plain words, short sentences,
specific about what happens. Warm, never cute. Confident, never salesy. The late-summer mood
lives in the visuals and in a rare light touch, not in every sentence.

- Say what the thing does, in the words people use. Capitalise the glossary terms in product
  copy: Library, Feed, Folder, Article, Layout, Starred.
- Active voice, sentence case, no exclamation marks.
- A button says exactly what happens, and the result uses the same verb: **Import OPML** →
  "Imported 42 Feeds."
- Errors say what happened and what to do next. They don't apologise and don't blame.
- Empty states invite the next step.
- One light touch per screen is plenty. The app's "all caught up" line is the model:
  *Enjoy the late-summer sun.*

### Examples

| Where | Write | Not |
|---|---|---|
| Hero headline | A calm reader for the Feeds you follow. | Unlock the power of RSS. |
| Subhead | Everything lives in one Library folder on your machine. No account, no sync, nothing phones home. | Your all-in-one seamless reading experience. |
| Portable Mode | Put your Library on a USB stick. Open it on a friend's laptop and pick up where you left off. | Take your data anywhere with cloud-free sync! |
| Primary button | Download for Linux | Get started now |
| Secondary button | See the keyboard shortcuts | Learn more |
| App buttons | Create new Library · Open existing Library… · Import OPML… · Add Feed | New workspace · Load · Upload · Subscribe |
| Error | This file isn't an OPML file. Export your Feeds as OPML from your old reader, then import that file. | Oops! Something went wrong. |
| Error | Couldn't reach this Feed. Check the address, or try again later. | Invalid feed URL. |
| Empty state | No Feeds yet. Import an OPML file or paste a website address to add your first Feed. | Nothing to see here! |
| All caught up | No unread Articles. Enjoy the late-summer sun. | You crushed it! Inbox zero! |
| Release note | In the Cards, Magazine and Gallery Layouts, Enter now opens the Article in the reader. | Exciting new improvements to your experience! |

### Words to avoid

- **Glossary avoid list:** profile, database, account, workspace (say Library); subscription,
  channel, source (Feed); item, entry, post, story (Article); saved, favourite, bookmarked
  (Starred); view mode, display (Layout); category, tag (Folder).
- **Marketing filler:** unlock, supercharge, seamless, effortless, powerful, revolutionary,
  next-generation, AI-powered, all-in-one, experience (as a noun), journey, empower.
- **Cloud words** that suggest what Feedrico doesn't do: sync, cloud, sign up, log in, account,
  except to say plainly that there is none.
- **Softeners and fake warmth:** just, simply, easily, oops, whoops, awesome, emoji.

## Ideas for the website

Suggestions for the website, each built on something only Feedrico can say. Pick one as the
memorable thing and keep the rest of the site quiet.

### 1. The light follows your clock

The site reads the visitor's **local time from the browser clock** (no geolocation, no
request, nothing stored) and sets the light to match. In the morning it is paper and pale
honey. In the afternoon the honey deepens and the threads in the hero catch more light. After
sunset it turns to dusk violet, the drop glows warmer, and the page reads like the app's dark
theme. A small, honest line explains it: *"It's 18:40 where you are. We only asked your
clock."* A toggle lets people pick a theme instead, and `prefers-color-scheme` wins until they
do. It shows the product's principle (local, private) through the design itself.

Build it by interpolating between the two token themes in four or five steps (morning,
midday, golden hour, dusk, night) and changing the step at most once per visit, never
animating while someone reads.

### 2. A Library goes to a friend's house

A scroll story in five steps with one recurring object: a USB stick with a paper tag that reads
"Library". At home on a desk, with 150 Feeds and a Starred Article. In a pocket. On a train,
offline, still readable. Plugged into a friend's laptop, where nothing gets installed. The
same Article, open at the same place, on someone else's screen. Each step pairs a line
drawing in the thread style with one plain fact (Portable Mode, Read state travels, no
account). The personas are the cast: the Carrier, the Offline Reader, the Host. Every Feed
name and Article title on screen is placeholder text.

### 3. Read the site the way you read in Feedrico

The landing page is laid out like an Article in the reader: Literata, a 66-character measure,
calm margins. It also answers to Feedrico's keyboard shortcuts. `j` and `k` move between
sections, `v` cycles the sections through List, Cards, Magazine and Gallery Layouts, and `s`
stars a section (the star stays in that tab only). A quiet hint in the corner shows the key
you can press next. By the end of the page, visitors have learned the shortcuts and have seen
all four Layouts with placeholder Articles, without anyone explaining either.

### Small touch: an honest footer

A footer line counts the network requests the page made, measured live with the Performance
API: *"This page made 9 requests, all to this site. Feedrico only talks to your Feeds."* It
proves the no-telemetry promise and costs nothing. It also keeps whoever builds the site
honest about analytics and font CDNs.

## Rebuilding

All SVGs come from `brand/tools/` (Python). The wordmark and social card text are outlined
from the bundled fonts in `node_modules/@fontsource-variable`.

```sh
# 1. SVG sources (needs fonttools, brotli and uharfbuzz; uv fetches them on the fly)
uv run --with fonttools --with brotli --with uharfbuzz python brand/tools/build_brand.py
# 2. PNG/ICO exports and src-tauri/icons (needs Inkscape and ImageMagick)
brand/tools/export.sh
```

| File | Builds |
|---|---|
| `tools/geometry.py` | Tapered threads (`ribbon`), arcs, curves |
| `tools/mark.py` | The mark in its three weights |
| `tools/wordmark.py` | Outlined wordmark with the drop and the strand |
| `tools/icon.py` | App icon masters and the shared colour table |
| `tools/motif.py` | Motif SVGs |
| `tools/social.py` | Social card |
| `tools/textpath.py` | Text to outlines (HarfBuzz shaping, fontTools outlines) |
| `tools/export.sh` | PNG, ICO and Tauri icons |

`export.sh` runs `npx tauri icon` on `app-icon.svg`, deletes the generated `android/` and
`ios/` folders, then replaces the 16–48 px layers with renders of `app-icon-small.svg`.
