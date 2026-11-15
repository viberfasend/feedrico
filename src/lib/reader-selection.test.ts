// @vitest-environment happy-dom
// The reader's text selection as input for "Explain a term" (#118): the selected text is the
// term, the paragraph around it the context, both capped, and only inside the reader pane.
import { describe, expect, it } from 'vitest';
import { MAX_CONTEXT_CHARS, MAX_TERM_CHARS, readerSelection } from './reader-selection';

function reader(html: string): HTMLElement {
  document.body.innerHTML = `<section id="pane-reader"><div class="content">${html}</div></section><input id="search">`;
  return document.getElementById('pane-reader')!;
}

function select(node: Node): Selection {
  const range = document.createRange();
  range.selectNodeContents(node);
  const sel = window.getSelection()!;
  sel.removeAllRanges();
  sel.addRange(range);
  document.dispatchEvent(new Event('selectionchange'));
  return sel;
}

describe('readerSelection', () => {
  it('returns the selected term and its surrounding paragraph', () => {
    reader('<p>First para.</p><p>The <b>quota</b> is a limit on requests.\nIt resets daily.</p>');
    select(document.querySelector('b')!);
    expect(readerSelection()).toEqual({
      term: 'quota',
      context: 'The quota is a limit on requests. It resets daily.',
    });
  });

  it('collapses whitespace in term and context', () => {
    reader('<p>  A   very\n\n  spread   out   selection  </p>');
    select(document.querySelector('p')!);
    const s = readerSelection()!;
    expect(s.term).toBe('A very spread out selection');
    expect(s.context).toBe('A very spread out selection');
  });

  it('uses the list item, not the whole list, as the context', () => {
    reader('<ul><li>one</li><li>the <em>term</em> here</li><li>three</li></ul>');
    select(document.querySelector('em')!);
    expect(readerSelection()!.context).toBe('the term here');
  });

  it('is null for a collapsed selection, an empty one, or one outside the reader', () => {
    reader('<p>Some text here.</p>');
    const p = document.querySelector('p')!;
    const collapsed = document.createRange();
    collapsed.setStart(p.firstChild!, 2);
    collapsed.collapse(true);
    const sel = window.getSelection()!;
    sel.removeAllRanges();
    sel.addRange(collapsed);
    expect(readerSelection()).toBeNull();

    select(document.getElementById('search')!);
    expect(readerSelection()).toBeNull();

    document.body.innerHTML = '<p>Outside the reader.</p>';
    select(document.querySelector('p')!);
    expect(readerSelection()).toBeNull();
  });

  it('caps a long term and a long paragraph', () => {
    const term = 'x'.repeat(MAX_TERM_CHARS * 3);
    const para = 'y'.repeat(MAX_CONTEXT_CHARS * 3);
    reader(`<p>${term}</p><p>${para}</p>`);
    select(document.querySelectorAll('p')[0]!);
    let s = readerSelection()!;
    expect(s.term).toHaveLength(MAX_TERM_CHARS);
    expect(s.context).toHaveLength(MAX_TERM_CHARS * 3); // under the context cap: sent whole
    select(document.querySelectorAll('p')[1]!);
    s = readerSelection()!;
    expect(s.term).toHaveLength(MAX_TERM_CHARS);
    expect(s.context).toHaveLength(MAX_CONTEXT_CHARS);
  });
});
