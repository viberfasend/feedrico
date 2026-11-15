// The reader's text selection, as input for "Explain a term" (#118): the selected text is the
// term, and the paragraph around it is the context the Assistant explains the term with. Both
// are capped here so a huge selection never crosses the IPC boundary; the core caps again.

/** Longest selection sent as the term. Mirrors MAX_TERM_CHARS in the core. */
export const MAX_TERM_CHARS = 400;
/** Longest surrounding paragraph sent as context. Mirrors MAX_CONTEXT_CHARS in the core. */
export const MAX_CONTEXT_CHARS = 4_000;

export interface ReaderSelection {
  term: string;
  context: string;
}

/** Block elements whose text is "the surrounding paragraph" of a selection. */
const BLOCK =
  'p, li, h1, h2, h3, h4, h5, h6, blockquote, pre, figcaption, td, [role="document"] > *';

function collapse(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}

function elementOf(node: Node | null): Element | null {
  if (node instanceof Element) return node;
  return node?.parentElement ?? null;
}

/**
 * The current selection inside the reader pane, capped, or null when there is none (collapsed,
 * empty, or outside the reader, for example in the search field).
 */
export function readerSelection(): ReaderSelection | null {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || sel.rangeCount === 0) return null;
  const range = sel.getRangeAt(0);
  const reader = document.getElementById('pane-reader');
  if (!reader || !reader.contains(range.commonAncestorContainer)) return null;
  const term = collapse(range.toString()).slice(0, MAX_TERM_CHARS);
  if (!term) return null;
  const block =
    elementOf(range.commonAncestorContainer)?.closest(BLOCK) ?? reader.querySelector('.content');
  const context = collapse(block?.textContent ?? '').slice(0, MAX_CONTEXT_CHARS);
  return { term, context };
}
