# Math in Articles: pass inert MathML through the sanitizer; raw TeX delimiters stay literal text

Issue #78 (parent #74). There is no RSS standard for math. Feeds carry pre-baked MathML, raw TeX
delimiters as text (`$…$`, `$$…$$`, `\(…\)`, `\[…\]`), KaTeX/MathJax server spans, or equation
images (which already work through the media cache). Two decisions:

1. **MathML passes the sanitizer.** `crates/feedrico-core/src/sanitize.rs` allow-lists the MathML
   Core element set (`math`, `semantics`, `annotation`, `mrow`, `mi`, `mn`, `mo`, `ms`, `mtext`,
   `mspace`, `mfrac`, `msqrt`, `mroot`, `msup`, `msub`, `msubsup`, `munder`, `mover`,
   `munderover`, `mmultiscripts`, `mprescripts`, `none`, `mtable`, `mtr`, `mlabeledtr`, `mtd`,
   `mstyle`, `menclose`, `mfenced`, `mpadded`, `mphantom`, `merror`, `maction`, `mstack`,
   `msrow`, `msgroup`, `msline`, `mscarry`, `mscarries`). Only two attributes survive, both on
   `math` and both plain text: `alttext` (the accessible name) and `display` (block vs inline).
   `annotation-xml` and `mglyph` stay out: `annotation-xml encoding="text/html"` can smuggle HTML
   into the MathML tree. With no URL attributes allowed, `maction actiontype`/`href` (the old
   `javascript:` statusline vector), `onclick`, `style`, `class` and every other handler are
   stripped like everywhere else — MathML has no script vector once script tags and URL attributes
   stay out. The reader (`Reader.svelte`) renders equations natively: display math
   (`math[display="block"]`) centered on its own scrollable line, inline math in the text flow,
   glyphs inheriting the reader font size and colored with `--color-text-default`, which the token
   build AA-checks against the reader surface in both themes.
2. **Raw TeX delimiters stay literal selectable text in v1.** No renderer, no CDN. The spec
   network policy (`docs/spec/v1.md`) allows bundled assets only — a CDN MathJax/KaTeX is out —
   and we do not bundle a local renderer in this change: it would add a JS library plus its fonts
   to the binary (Portable Mode on a stick, ADR 0003) to serve the niche of blogs that ship raw
   TeX, arXiv-style Feeds carry no TeX at all, equation images already render through the media
   cache, and KaTeX/MathJax server spans degrade to their text content. `$…$` therefore stays
   selectable, copyable text (already true before this change). If a real Library shows many
   raw-TeX Feeds, revisit with a bundled KaTeX (no read-time requests, fonts shipped with the app);
   that would be a new ADR superseding this one.

## Personas

- **Accessibility spectrum, "can't see the screen well"** (`docs/personas.md`): permanent — a
  blind user with a screen reader. MathML is the only math format NVDA/JAWS/VoiceOver can
  navigate and voice; the webview gives `math` its implicit role `math` and takes the accessible
  name from `alttext`, which is exactly why the sanitizer keeps those two attributes. Situational —
  low vision and bright sunlight: equations are real text, so they scale with the reader
  typography (Settings font size/measure/line height) and keep AA contrast in both themes.
- **The Archivist (Henrik)** and **The Skimmer (Clara)** reading math blogs: equations no longer
  vanish from Articles; raw TeX at worst stays literal text, so nobody is hurt by decision 2.
- **The Offline Reader (Paul)**: unchanged — rendering MathML is local, opening an Article still
  makes no network request.

## Consequences

- Sanitized Article HTML can contain MathML; the webview's native MathML renderer and
  accessibility tree do the work, no JS math engine is shipped.
- Tests: a MathML fixture survives `sanitize` (equation renders), math with `onclick` and
  `maction` `actiontype`/`href="javascript:"` is stripped safely, a raw-TeX fixture keeps `$…$` as
  text (`crates/feedrico-core/src/sanitize.rs`), and the reader renders MathML with `alttext`/
  `display` and styles display math centered (`src/lib/components/reader.test.ts`).
- If we later bundle KaTeX for raw TeX, it must ship with the app (no CDN, no read-time request)
  and this ADR gets superseded, not silently changed.
