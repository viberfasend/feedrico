// @vitest-environment happy-dom
// Reader content: images that aren't in the local cache.
//
// Bug (#18): an image skipped for size (over 5 MB) or evicted from the cache vanished from the
// Article. The spec says it "stays a remote link": the reader replaces it with a link to the
// original, which opens in the system browser. Opening the Article still loads nothing remote.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ArticleDetail, VideoMeta } from '../api';

const mockApi = { openExternal: vi.fn(async (_url: string) => {}) };

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
  mediaUrl: (key: string) => `media://localhost/${key}`,
}));

const { app } = await import('../state.svelte');
const { default: Reader } = await import('./Reader.svelte');
const { default: readerSource } = await import('./Reader.svelte?raw');

const article = (contentHtml: string): ArticleDetail => ({
  id: 1,
  feedId: 1,
  feedTitle: 'Blog',
  title: 'Post',
  excerpt: '',
  articleDate: 1_700_000_000,
  isRead: false,
  isStarred: false,
  leadImage: null,
  favicon: null,
  feedSiteUrl: null,
  url: null,
  author: null,
  contentHtml,
  enclosures: [],
});

let mounted: ReturnType<typeof mount> | null = null;

function render(contentHtml: string) {
  app.article = article(contentHtml);
  mounted = mount(Reader, { target: document.body });
  flushSync();
  return document.querySelector('.content') as HTMLElement;
}

function failToLoad(img: Element) {
  img.dispatchEvent(new Event('error'));
  flushSync();
}

afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  app.article = null;
  document.body.innerHTML = '';
  mockApi.openExternal.mockClear();
});

describe('Reader images that are not cached', () => {
  it('become a link to the original that opens in the browser', () => {
    const content = render(
      '<p><img src="media://localhost/k" data-remote="https://blog.example/a.png" alt="Chart"></p>',
    );
    failToLoad(content.querySelector('img')!);

    expect(content.querySelector('img')).toBeNull();
    const link = content.querySelector('.remote-image a') as HTMLAnchorElement;
    expect(link.getAttribute('href')).toBe('https://blog.example/a.png');
    expect(link.textContent).toContain('blog.example');
    expect(content.querySelector('.remote-image')?.textContent).toContain('Chart');

    link.click();
    expect(mockApi.openExternal).toHaveBeenCalledWith('https://blog.example/a.png');
  });

  it('never nest a link inside the link that already wraps the image', () => {
    const content = render(
      '<a href="https://blog.example/post"><img src="media://localhost/k" data-remote="https://blog.example/b.png"></a>',
    );
    failToLoad(content.querySelector('img')!);

    const outer = content.querySelector('a') as HTMLAnchorElement;
    expect(outer.querySelector('a')).toBeNull();
    expect(outer.querySelector('.remote-image')?.textContent).toContain('not saved for offline reading');
  });

  it('stay hidden when the Article predates the original URL being kept', () => {
    const content = render('<img src="media://localhost/k">');
    const img = content.querySelector('img')!;
    failToLoad(img);
    expect(img.classList.contains('missing')).toBe(true);
  });

  it('only link to http(s) originals', () => {
    const content = render('<img src="media://localhost/k" data-remote="javascript:alert(1)">');
    const img = content.querySelector('img')!;
    failToLoad(img);
    expect(content.querySelector('.remote-image')).toBeNull();
    expect(img.classList.contains('missing')).toBe(true);
  });
});

const detail = (overrides: Partial<ArticleDetail>): ArticleDetail => ({ ...article(''), ...overrides });

function renderArticle(overrides: Partial<ArticleDetail>) {
  app.article = detail(overrides);
  mounted = mount(Reader, { target: document.body });
  flushSync();
  return document.getElementById('pane-reader') as HTMLElement;
}

describe('Reader images by size', () => {
  it('turn emoji images back into text (WordPress wp-smiley)', () => {
    const content = render('<p>We loved it <img src="media://localhost/e" alt="😀" width="72" height="72"> really</p>');
    expect(content.querySelector('img')).toBeNull();
    expect(content.querySelector('.emoji')?.textContent).toBe('😀');
  });

  it('hide tracking pixels and keep share icons in the line of text', () => {
    const content = render(
      '<p>Share: <img src="media://localhost/s" width="24" height="24" alt="Share"></p><img src="media://localhost/p" width="1" height="1">',
    );
    const [icon, pixel] = content.querySelectorAll('img');
    expect(icon!.classList.contains('img-inline')).toBe(true);
    expect(icon!.getAttribute('role')).toBeNull();
    expect(pixel!.classList.contains('img-pixel')).toBe(true);
    expect(pixel!.getAttribute('aria-hidden')).toBe('true');
  });

  it('show comic title text as a caption, but not twice under a figure caption', () => {
    const content = render(
      '<p><img src="media://localhost/c" width="740" height="2400" title="The joke" alt="Comic"></p>' +
        '<figure><img src="media://localhost/f" width="800" height="500" title="Hover"><figcaption>Caption</figcaption></figure>',
    );
    expect([...content.querySelectorAll('.img-caption')].map((c) => c.textContent)).toEqual(['The joke']);
    // A tall comic is meant to be scrolled: no height cap.
    expect(content.querySelector('img')!.classList.contains('img-cap')).toBe(false);
  });

  // Bug: news feeds (Saarbrücker Zeitung) send `<img align="left">` before the teaser text, and
  // the sanitizer's allow-list keeps `align`. The webview turns it into `float: left`, the
  // block's `margin: auto` becomes 0 beside a float, and the text ran right up against the
  // picture. The spec shows every picture as a centred block, so the reader ignores `align`.
  // Stored Articles already carry the attribute, so this has to hold in the reader's CSS.
  it.each(['img', 'table'])('never float %s elements that carry an align attribute', (tag) => {
    const rule = new RegExp(`\\.content :global\\(${tag}\\) \\{[^}]*\\bfloat: none;`);
    expect(readerSource).toMatch(rule);
  });
});

describe('Image viewer', () => {
  it('opens on Enter, browses with arrows, and Esc closes only the viewer', async () => {
    app.readerOpen = true;
    const content = render(
      '<img src="media://localhost/a" width="1600" height="1000" alt="First"><img src="media://localhost/b" width="1600" height="1000" alt="Second">',
    );
    const first = content.querySelector('img')!;
    expect(first.getAttribute('role')).toBe('button');
    expect(first.getAttribute('aria-label')).toBe('View image: First');

    first.focus();
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    flushSync();
    const viewer = document.querySelector('.viewer[role="dialog"]') as HTMLElement;
    expect(viewer).not.toBeNull();
    expect(viewer.querySelector('.count')?.textContent).toContain('1 / 2');

    const windowKeys = vi.fn();
    window.addEventListener('keydown', windowKeys);
    viewer.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    flushSync();
    expect(viewer.querySelector('img')?.getAttribute('alt')).toBe('Second');

    viewer.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    flushSync();
    window.removeEventListener('keydown', windowKeys);
    expect(document.querySelector('.viewer')).toBeNull();
    // The reader behind it never saw the keys: it stays open on the same Article.
    expect(windowKeys).not.toHaveBeenCalled();
    expect(app.readerOpen).toBe(true);
    app.readerOpen = false;
  });

  it('leaves linked images to their link (links open in the browser)', () => {
    const content = render('<a href="https://blog.example/full"><img src="media://localhost/l" width="1200" height="800"></a>');
    expect(content.querySelector('img')!.getAttribute('role')).toBeNull();
  });
});

describe('Lead Image in the reader', () => {
  it('leads a summary-only Article, so the reader shows what the card showed', () => {
    const pane = renderArticle({ leadImage: 'lead1', contentHtml: '<p>Read the full story on our website.</p>' });
    expect(pane.querySelector('.lead img')).not.toBeNull();
  });

  it('is not shown twice when the content has its own picture', () => {
    const pane = renderArticle({
      leadImage: 'lead1',
      contentHtml: '<p><img src="media://localhost/other" width="1024" height="683"></p><p>Text</p>',
    });
    expect(pane.querySelector('.lead')).toBeNull();
  });
});

describe('Attachments', () => {
  // Bug: enclosure links sat outside the content's click handler, and the shell's navigation
  // guard blocks the webview from leaving the app, so clicking an attachment did nothing.
  it('open in the browser when clicked', () => {
    const pane = renderArticle({
      enclosures: [{ url: 'https://cdn.example/ep88.mp3?utm=rss', mimeType: 'audio/mpeg', length: 58_400_000, durationSecs: null, kind: 'audio', playerUrl: null }],
    });
    const link = pane.querySelector('a[href="https://cdn.example/ep88.mp3?utm=rss"]') as HTMLAnchorElement;
    link.click();
    expect(mockApi.openExternal).toHaveBeenCalledWith('https://cdn.example/ep88.mp3?utm=rss');
  });

  it('read as name and kind, and hide an image enclosure that is the Lead Image', () => {
    const pane = renderArticle({
      leadImage: 'cover',
      enclosures: [
        { url: 'https://cdn.example/audio/ep%2088.mp3?utm=rss', mimeType: 'audio/mpeg', length: 58_400_000, durationSecs: null, kind: 'audio', playerUrl: null },
        { url: 'https://cdn.example/art/88.jpg', mimeType: 'image/jpeg', length: 412_000, durationSecs: null, kind: 'document', playerUrl: null },
      ],
    });
    const items = [...pane.querySelectorAll('[aria-label="Attachments"] li')].map((li) => li.textContent?.replace(/\s+/g, ' ').trim());
    expect(items).toHaveLength(1);
    expect(items[0]).toContain('ep 88.mp3');
    expect(items[0]).toContain('Audio · MP3 · 55.7 MB');
  });
});

describe('Attachment details from MediaRSS / JSON Feed metadata', () => {
  // The Archivist (Henrik) wants the episode length at a glance (#76).
  const items = (pane: HTMLElement) =>
    [...pane.querySelectorAll('[aria-label="Attachments"] li')].map((li) => li.textContent?.replace(/\s+/g, ' ').trim());

  it('show kind, duration, format and size next to the name', () => {
    const pane = renderArticle({
      enclosures: [{ url: 'https://pod.example/ep12.mp3', mimeType: 'audio/mpeg', length: 12_000_000, durationSecs: 754, kind: 'audio', playerUrl: null }],
    });
    expect(items(pane)[0]).toContain('ep12.mp3');
    expect(items(pane)[0]).toContain('Audio · MP3 · 12:34 · 11.4 MB');
  });

  it('format durations over an hour as h:mm:ss', () => {
    const pane = renderArticle({
      enclosures: [{ url: 'https://v.example/lecture.mp4', mimeType: 'video/mp4', length: null, durationSecs: 3725, kind: 'video', playerUrl: null }],
    });
    expect(items(pane)[0]).toContain('Video · MP4 · 1:02:05');
  });

  it('open the media:player page when there is no direct media URL', () => {
    const pane = renderArticle({
      enclosures: [{ url: 'https://cdn.example/v/xyz', mimeType: null, length: null, durationSecs: null, kind: 'document', playerUrl: 'https://player.example/watch?v=xyz' }],
    });
    const link = pane.querySelector('[aria-label="Attachments"] a') as HTMLAnchorElement;
    expect(link.getAttribute('href')).toBe('https://player.example/watch?v=xyz');
    expect(items(pane)[0]).toContain('Player');
  });
});

describe('Title-only Articles', () => {
  it('say so instead of showing an empty page', () => {
    const pane = renderArticle({ url: 'https://blog.example/post', contentHtml: '' });
    expect(pane.querySelector('.no-text')?.textContent).toContain('only a title');
    expect(pane.querySelector('.end')?.textContent).toContain('Open on blog.example');
  });
});

// Issue #77: code blocks in the reader keep their language-* class (sanitizer), get highlighted
// locally by the bundled highlighter (no CDN, no network), and each pre block gets a copy button.
// Personas: the Archivist (Henrik) reads dev blogs in batches; the accessibility spectrum needs
// the button keyboard-operable with a visible focus ring and an accessible name.
describe('Reader code blocks', () => {
  const RUST_BLOCK =
    '<p>Intro</p><pre><code class="language-rust">fn main() {\n    greet("hi"); // greet\n}</code></pre>';
  const RUST_CODE = 'fn main() {\n    greet("hi"); // greet\n}';

  function mockClipboard() {
    const writeText = vi.fn(async (_text: string) => {});
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    });
    return writeText;
  }

  it('keeps the language class and highlights the block locally with token spans', () => {
    const content = render(RUST_BLOCK);
    const code = content.querySelector('pre code') as HTMLElement;
    expect(code.className).toBe('language-rust');
    expect(code.querySelector('.tok-keyword')?.textContent).toBe('fn');
    const functions = [...code.querySelectorAll('.tok-function')].map((n) => n.textContent);
    expect(functions).toContain('greet');
    expect(functions).toContain('main');
    expect(code.querySelector('.tok-string')?.textContent).toBe('"hi"');
    expect(code.querySelector('.tok-comment')?.textContent).toBe('// greet');
    // Highlighting must not change the code text (copy + search rely on it).
    expect(code.textContent).toBe(RUST_CODE);
  });

  it('adds a keyboard-operable copy button with an accessible name to each pre block', async () => {
    const writeText = mockClipboard();
    const content = render(RUST_BLOCK);
    const button = content.querySelector('pre button.code-copy') as HTMLButtonElement;
    expect(button).toBeTruthy();
    expect(button.tagName).toBe('BUTTON');
    expect(button.type).toBe('button');
    expect(button.getAttribute('aria-label')).toBe('Copy rust code block');
    // Keyboard-operable: focusable via Tab, activated with Enter/Space (native button behavior).
    button.focus();
    expect(document.activeElement).toBe(button);
    button.click();
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    // Copies the code only, never the button label.
    expect(writeText).toHaveBeenCalledWith(RUST_CODE);
    await vi.waitFor(() => expect(button.textContent).toBe('Copied'));
  });

  it('wraps a bare pre in code and still offers a copy button', () => {
    const content = render('<pre>plain code</pre>');
    const pre = content.querySelector('pre') as HTMLElement;
    expect(pre.querySelector('code')?.textContent).toBe('plain code');
    // No language class: no highlighting, but the boxed block and button are still there.
    expect(pre.querySelectorAll('[class^="tok-"]').length).toBe(0);
    const button = pre.querySelector('button.code-copy') as HTMLButtonElement;
    expect(button.getAttribute('aria-label')).toBe('Copy code block');
  });

  it('never breaks the reader width: the pre scrolls horizontally instead of wrapping', () => {
    const content = render(
      '<pre><code class="language-rust">let x = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";</code></pre>',
    );
    const pre = content.querySelector('pre') as HTMLElement;
    const style = getComputedStyle(pre);
    expect(style.overflowX).toBe('auto');
    expect(getComputedStyle(pre.querySelector('code')!).whiteSpace).toBe('pre');
    expect(getComputedStyle(pre.querySelector('code')!).overflowWrap).toBe('normal');
  });
});

// Math in Articles (issue #78, ADR 0004): the Rust core passes inert MathML through the
// sanitizer (role "math" is implicit in the webview, the accessible name comes from alttext),
// and the reader styles it. Raw TeX delimiters stay literal selectable text — no renderer.
describe('Reader math', () => {
  it('renders MathML equations with their alttext name and display mode', () => {
    const content = render(
      '<p>Energy:</p>' +
        '<math alttext="E equals m c squared" display="block"><mrow><mi>E</mi><mo>=</mo><mi>m</mi><msup><mi>c</mi><mn>2</mn></msup></mrow></math>' +
        '<p>Inline <math alttext="a squared plus b squared equals c squared"><mrow><msup><mi>a</mi><mn>2</mn></msup><mo>+</mo><msup><mi>b</mi><mn>2</mn></msup><mo>=</mo><msup><mi>c</mi><mn>2</mn></msup></mrow></math> stays in flow.</p>',
    );
    const block = content.querySelector('math[display="block"]');
    expect(block).not.toBeNull();
    // The screen-reader path: alttext is the accessible name of role "math".
    expect(block!.getAttribute('alttext')).toBe('E equals m c squared');
    expect(block!.querySelector('msup mn')?.textContent).toBe('2');
    // Inline math has no display attribute and stays in the text flow.
    const inline = content.querySelectorAll('math:not([display="block"])');
    expect(inline.length).toBe(1);
    expect(inline.item(0).getAttribute('alttext')).toBe('a squared plus b squared equals c squared');
  });

  it('keeps raw TeX delimiters as literal selectable text', () => {
    const content = render('<p>Inline $E = mc^2$ and $$x = \\\\frac{1}{2}$$ stay text.</p>');
    expect(content.querySelector('math')).toBeNull();
    expect(content.textContent).toContain('$E = mc^2$');
    expect(content.textContent).toContain('$$x = \\\\frac{1}{2}$$');
  });

  it('styles math: display math centered as a block, colored with the AA text token', () => {
    render('<math display="block"><mrow><mi>x</mi></mrow></math>');
    const css = Array.from(document.querySelectorAll('style'))
      .map((s) => s.textContent ?? '')
      .join('\n');
    // Display math: own block line, centered, scrollable instead of breaking the measure.
    expect(css).toMatch(/math\[display="block"\]\s*\{[^}]*display:\s*block/s);
    expect(css).toMatch(/math\[display="block"\]\s*\{[^}]*text-align:\s*center/s);
    // Math glyphs use the text color token, AA-checked against the reader surface in both
    // themes, and inherit the reader font size (em), so equations scale with the typography.
    expect(css).toMatch(/math\s*\{[^}]*color:\s*var\(--color-text-default\)/s);
  });
});

describe('Video card (#75)', () => {
  // A video Feed Article: media:thumbnail is the Lead Image and cached at fetch time.
  const video: VideoMeta = {
    host: 'youtube',
    id: '0A1ouV7iD8o',
    duration: 754,
    views: 304_321,
    thumbnail: 'https://i1.ytimg.com/vi/0A1ouV7iD8o/hqdefault.jpg',
  };

  it('renders thumbnail, title, duration, views and open-in-browser from cached data only', () => {
    const pane = renderArticle({ video, leadImage: 'thumb1', contentHtml: '<p>A video post</p>' });
    const card = pane.querySelector('.video-card') as HTMLElement;
    expect(card).not.toBeNull();
    const img = card.querySelector('img.video-thumb') as HTMLImageElement;
    expect(img.getAttribute('src')).toBe('media://localhost/thumb1');
    expect(img.dataset.remote).toBe(video.thumbnail);
    expect(card.textContent).toContain('Post');
    expect(card.textContent).toContain('12:34');
    expect(card.textContent).toContain('304,321 views');
    const link = card.querySelector('a.video-open') as HTMLAnchorElement;
    expect(link.getAttribute('href')).toBe('https://www.youtube.com/watch?v=0A1ouV7iD8o');
    // Reading makes no network request (Offline Reader): no iframe, every img src is the cache.
    expect(pane.querySelector('iframe')).toBeNull();
    for (const el of pane.querySelectorAll('img')) {
      expect(el.getAttribute('src')!.startsWith('media://localhost/')).toBe(true);
    }
    link.click();
    expect(mockApi.openExternal).toHaveBeenCalledWith('https://www.youtube.com/watch?v=0A1ouV7iD8o');
  });

  it('shows the thumbnail once: not again as the Lead Image', () => {
    const pane = renderArticle({ video, leadImage: 'thumb1', contentHtml: '<p>A video post</p>' });
    expect(pane.querySelector('.video-card img.video-thumb')).not.toBeNull();
    expect(pane.querySelector('.lead')).toBeNull();
  });

  it('falls back to the remote-link behaviour when the thumbnail is not cached', () => {
    const pane = renderArticle({ video, leadImage: 'thumb1', contentHtml: '<p>A video post</p>' });
    failToLoad(pane.querySelector('.video-card img.video-thumb')!);
    expect(pane.querySelector('.video-card img')).toBeNull();
    const link = pane.querySelector('.video-card .remote-image a') as HTMLAnchorElement;
    expect(link.getAttribute('href')).toBe(video.thumbnail);
    link.click();
    expect(mockApi.openExternal).toHaveBeenCalledWith(video.thumbnail);
  });

  it('renders without a thumbnail when the Feed carries none (blog embed)', () => {
    const vimeo: VideoMeta = { host: 'vimeo', id: '76979871', duration: null, views: null, thumbnail: null };
    const pane = renderArticle({ video: vimeo, contentHtml: '<p>Embedded below</p>' });
    const card = pane.querySelector('.video-card') as HTMLElement;
    expect(card.querySelector('img')).toBeNull();
    expect(card.textContent).toContain('vimeo.com');
    expect(card.textContent).not.toContain('views');
    const link = card.querySelector('a.video-open') as HTMLAnchorElement;
    expect(link.getAttribute('href')).toBe('https://vimeo.com/76979871');
  });

  it('keeps the inline embed-card placeholder as a link card that opens in the browser', () => {
    const content = render(
      '<p class="embed-card" data-video-host="youtube" data-video-id="0A1ouV7iD8o" data-video-src="https://www.youtube.com/embed/0A1ouV7iD8o"><a href="https://www.youtube.com/embed/0A1ouV7iD8o">Video on youtube.com: open in browser</a></p>',
    );
    const placeholder = content.querySelector('.embed-card')!;
    expect(placeholder.getAttribute('data-video-host')).toBe('youtube');
    expect(placeholder.getAttribute('data-video-id')).toBe('0A1ouV7iD8o');
    expect(placeholder.getAttribute('data-video-src')).toBe('https://www.youtube.com/embed/0A1ouV7iD8o');
    const link = placeholder.querySelector('a') as HTMLAnchorElement;
    link.click();
    expect(mockApi.openExternal).toHaveBeenCalledWith('https://www.youtube.com/embed/0A1ouV7iD8o');
  });

  it('renders no card for a plain Article', () => {
    const pane = renderArticle({ contentHtml: '<p>Just text</p>' });
    expect(pane.querySelector('.video-card')).toBeNull();
  });
});
