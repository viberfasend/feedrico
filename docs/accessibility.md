# Accessibility audit (WCAG 2.2 AA)

Issue #32. Every screen was audited against WCAG 2.2 AA: Welcome, Attention, sidebar, the four
Layouts, slide-over reader, Reader, Add Feed, Settings, command palette, menus, toasts, confirm
dialogs. Automated coverage: the token build fails on any text pair below 4.5:1 and any non-text
pair below 3:1 in both themes (`tokens/contrast-pairs.json`), `scripts/motion-guard.test.mjs`
fails on raw animation durations, and `src/lib/components/axe.test.ts` runs axe-core over the
mounted app in every Layout with the reader, Settings and palette open (color-contrast disabled
there — no real layout in a DOM emulator; the token build owns contrast).

## Success criteria

| SC | Requirement | How it is met | Human check |
|---|---|---|---|
| 1.3.1 | Info and relationships | Landmarks: one `main` per screen (Welcome, Attention, app shell), `nav` sidebar, labelled `section`s for list and reader; listbox/option roles on Article rows; labels on every control | Screen reader pass |
| 1.4.3 | Contrast (minimum) text 4.5:1 | Enforced by the token build in both themes (`scripts/build-tokens.mjs`, `contrast-pairs.json`) | — |
| 1.4.11 | Non-text contrast 3:1 | Token build now also checks focus ring, input borders, unread dot and selection indicators (`border.focus`/`border.default`/`unread.indicator` pairs); selected rows and cards use a `border-focus` edge, not just a background | — |
| 1.4.12 | Text spacing | Reader typography is user-adjustable (font size, measure, line height in Settings) | — |
| 2.1.1 | Keyboard | Full keyboard operation: `j`/`k`, arrows, `s`/`m`/`o`/`r`/`v`, `/`, `Enter`/`Esc`, Ctrl/⌘+K, Ctrl/⌘+`,`, F6/Shift+F6 between panes | Keyboard-only pass |
| 2.1.2 | No keyboard trap | Dialogs, menus and the palette close with Esc and return focus (Dialog wrapper and `focusOnOpen` in `App.svelte`) | — |
| 2.4.3 | Focus order | DOM order sidebar → list → reader; F6/Shift+F6 cycles panes; focus moves into the slide-over reader on open and returns to the selected row on Esc | — |
| 2.4.6 | Headings and labels | Dialog titles, labelled panes, labelled icon buttons (`aria-label`), labelled inputs and selects | — |
| 2.4.7 | Focus visible | One global `:focus-visible` ring from tokens (`--color-border-focus`, 3:1 checked) | — |
| 2.4.11 | Focus not obscured | The Article list header is a separate grid row above the scroll viewport, so a focused row can never sit under it; the keep-in-view effect scrolls it fully into view | — |
| 2.5.8 | Target size (24×24) | Component tokens keep icon buttons at 28 px in compact density; the sidebar disclosure button was 16 px wide and now matches the row height (26 px compact) | Spot-check at 200% zoom |
| 3.2.6 | Consistent help | — | n/a in v1 |
| 4.1.2 | Name, role, value | listbox/option with `aria-selected`, `aria-setsize`/`aria-posinset` on every rendered row ("5 of 1,240"), `aria-pressed` on toggles, `role="status"` live region on toasts and the "N Articles without images" note, `role="progressbar"` on refresh, `role="alert"` on Add Feed errors | Screen reader pass |
| 4.1.3 | Status messages | Toasts and the gallery note are in a polite live region; refresh progress is a progressbar | — |

## Known gaps and manual checks

- Screen readers were not run in CI: verify with Orca (Linux), NVDA (Windows) and VoiceOver
  (macOS) that the virtualized rows announce "N of M" correctly while scrolling, and that the
  sidebar counts read "12 unread".
- `prefers-reduced-motion` is honoured globally in `src/app.css` (all durations to 0.01 ms) and
  the motion guard keeps new motion inside duration tokens; verify with the OS setting on.
- 2.5.8 has a spacing exception in WCAG; we meet the 24 px minimum outright except where a target
  is inside a dense 26 px row — spot-check at 200% zoom.
- The Settings dialog's live preview line is decorative; its values are also exposed through the
  labelled inputs.

## Personas

Covers the whole accessibility spectrum in `docs/personas.md`: keyboard-only (motor), screen
reader and low vision (font size, measure, line height, AA contrast in both themes), limited
attention (calm UI, live-region status instead of surprise motion).
