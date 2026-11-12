// @vitest-environment happy-dom
// Reader content: images that aren't in the local cache.
//
// Bug (#18): an image skipped for size (over 5 MB) or evicted from the cache vanished from the
// Article. The spec says it "stays a remote link": the reader replaces it with a link to the
// original, which opens in the system browser. Opening the Article still loads nothing remote.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ArticleDetail } from '../api';

const mockApi = { openExternal: vi.fn(async (_url: string) => {}) };

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

const { app } = await import('../state.svelte');
const { default: Reader } = await import('./Reader.svelte');

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
    expect(link.textContent).toContain('Chart');

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
    expect(outer.querySelector('.remote-image')?.textContent).toContain('blog.example');
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
