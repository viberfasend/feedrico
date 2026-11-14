// @vitest-environment happy-dom
// An Assistant's output is untrusted: the renderer emits only paragraphs, lists, bold, italic
// and http(s) links, and escapes everything else.
import { describe, expect, it } from 'vitest';
import { renderMarkdown } from './assistant-markdown';

describe('renderMarkdown', () => {
  it('renders paragraphs, lists, bold, italic and links', () => {
    const html = renderMarkdown(
      'First **bold** and *italic*.\n\nSecond [link](https://example.com/a?x=1&y=2).\n\n- one\n- two\n\n1. a\n2. b',
    );
    expect(html).toContain('<p>First <strong>bold</strong> and <em>italic</em>.</p>');
    expect(html).toContain('<a href="https://example.com/a?x=1&amp;y=2" rel="noopener noreferrer">link</a>');
    expect(html).toContain('<ul><li>one</li><li>two</li></ul>');
    expect(html).toContain('<ol><li>a</li><li>b</li></ol>');
  });

  it('escapes script tags and event handlers', () => {
    const html = renderMarkdown('<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>');
    expect(html).not.toContain('<script');
    expect(html).not.toContain('<img');
    expect(html).toContain('&lt;script&gt;');
    const doc = new DOMParser().parseFromString(html, 'text/html');
    expect(doc.querySelector('script, img')).toBeNull();
    expect(doc.querySelector('[onerror]')).toBeNull();
  });

  it('drops javascript:, data: and other non-http links but keeps their text', () => {
    for (const url of ['javascript:alert(1)', 'JaVaScRiPt:alert(1)', 'data:text/html,hi', 'vbscript:x', ' javascript:alert(1)']) {
      const html = renderMarkdown(`[click me](${url})`);
      expect(html, url).not.toContain('<a');
      expect(html, url).not.toContain('href');
      expect(html, url).toContain('click me');
    }
  });

  it('cannot break out of the href attribute', () => {
    const html = renderMarkdown('[x](https://e.com/" onmouseover="alert(1))');
    const doc = new DOMParser().parseFromString(html, 'text/html');
    expect(doc.querySelector('[onmouseover]')).toBeNull();
  });

  it('returns an empty string for empty text', () => {
    expect(renderMarkdown('  \n ')).toBe('');
  });
});
