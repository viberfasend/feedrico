// @vitest-environment happy-dom
// Feed icons (#61): the cached favicon, or a generated tile when there is none or it won't load.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (key: string, protocol: string) => `${protocol}://localhost/${key}`,
  invoke: vi.fn(),
}));

const { default: FeedIcon } = await import('./FeedIcon.svelte');
const { FEED_TILES, feedInitial, feedTile } = await import('./feed-icon');

let mounted: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

function render(props: { feedId: number; title: string; favicon: string | null }) {
  mounted = mount(FeedIcon, { target: document.body, props });
  flushSync();
  return document.body;
}

describe('FeedIcon', () => {
  it('shows the cached favicon from the media protocol only', () => {
    const key = 'a'.repeat(32);
    const img = render({ feedId: 1, title: 'Blog', favicon: key }).querySelector('img');
    expect(img?.getAttribute('src')).toBe(`media://localhost/${key}`);
    expect(img?.getAttribute('alt')).toBe('');
  });

  it('shows a lettered tile when the Feed has no favicon', () => {
    const body = render({ feedId: 3, title: 'the Verge', favicon: null });
    expect(body.querySelector('img')).toBeNull();
    const tile = body.querySelector('.tile') as HTMLElement;
    expect(tile.textContent).toBe('T');
    expect(tile.getAttribute('aria-hidden')).toBe('true');
    expect(tile.style.background).toContain(`--color-feed-tile-${feedTile(3)}`);
  });

  it('falls back to the tile instead of a broken image when the file is missing', () => {
    const body = render({ feedId: 1, title: 'Blog', favicon: 'b'.repeat(32) });
    body.querySelector('img')!.dispatchEvent(new Event('error'));
    flushSync();
    expect(body.querySelector('img')).toBeNull();
    expect(body.querySelector('.tile')?.textContent).toBe('B');
  });
});

describe('tile helpers', () => {
  it('gives every Feed a stable tile colour within the palette', () => {
    for (const id of [0, 1, 5, 6, 7, 1234, -1]) {
      const tile = feedTile(id);
      expect(tile).toBeGreaterThanOrEqual(1);
      expect(tile).toBeLessThanOrEqual(FEED_TILES);
      expect(feedTile(id)).toBe(tile);
    }
    expect(new Set([1, 2, 3, 4, 5, 6].map(feedTile)).size).toBe(FEED_TILES);
  });

  it('uses the first letter or digit of the title', () => {
    expect(feedInitial('rust Blog')).toBe('R');
    expect(feedInitial('  "Ölwechsel" weekly')).toBe('Ö');
    expect(feedInitial('99% Invisible')).toBe('9');
    expect(feedInitial('---')).toBe('#');
  });
});
