// A small Markdown subset for Assistant output: paragraphs, lists, bold, italic and links.
//
// The output is untrusted, so this is a strict escaper rather than a parser that passes HTML
// through: every character is escaped first, and only the tags below are emitted. Links must be
// http(s); anything else (javascript:, data:, …) keeps its text and loses the link.

const ESCAPES: Record<string, string> = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

function escape(text: string): string {
  return text.replace(/[&<>"']/g, (c) => ESCAPES[c]!);
}

/** Bold, italic and links on text that is already escaped. */
function inline(escaped: string): string {
  return escaped
    .replace(/\[([^\]]+)\]\(((?:[^()\s]|\([^()\s]*\))+)\)/g, (_m, label: string, url: string) =>
      // `"` and `'` are escaped already, so the URL cannot leave the attribute.
      /^https?:\/\//i.test(url) ? `<a href="${url}" rel="noopener noreferrer">${label}</a>` : label,
    )
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^*])\*([^*\s][^*]*)\*(?!\*)/g, '$1<em>$2</em>');
}

const BULLET = /^\s*[-*]\s+(.*)$/;
const NUMBERED = /^\s*\d+[.)]\s+(.*)$/;

/** Renders `text` to sanitized HTML. */
export function renderMarkdown(text: string): string {
  const out: string[] = [];
  let paragraph: string[] = [];
  let list: { tag: 'ul' | 'ol'; items: string[] } | null = null;

  const flushParagraph = () => {
    if (paragraph.length) out.push(`<p>${paragraph.map((l) => inline(escape(l))).join('<br>')}</p>`);
    paragraph = [];
  };
  const flushList = () => {
    if (list) out.push(`<${list.tag}>${list.items.map((i) => `<li>${inline(escape(i))}</li>`).join('')}</${list.tag}>`);
    list = null;
  };

  for (const line of text.replace(/\r\n?/g, '\n').split('\n')) {
    const bullet = BULLET.exec(line);
    const numbered = bullet ? null : NUMBERED.exec(line);
    const item = bullet ?? numbered;
    if (item) {
      flushParagraph();
      const tag = bullet ? 'ul' : 'ol';
      if (list && list.tag !== tag) flushList();
      list ??= { tag, items: [] };
      list.items.push(item[1]!);
    } else if (!line.trim()) {
      flushParagraph();
      flushList();
    } else {
      flushList();
      paragraph.push(line.trim());
    }
  }
  flushParagraph();
  flushList();
  return out.join('');
}
