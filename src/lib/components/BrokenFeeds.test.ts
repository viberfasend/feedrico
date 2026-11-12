// @vitest-environment happy-dom
// Broken Feeds dialog (#66): one place to see broken Feeds with their Folder, last error and
// last success, retry one now, or unsubscribe (one, or all listed after a confirmation). It is
// offered only while there are broken Feeds, and stays light with hundreds of them.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FeedNode, ListQuery, Sidebar } from '../api';

const feed = (id: number, title: string, lastError: string | null, extra: Partial<FeedNode> = {}): FeedNode => ({
  id,
  title,
  siteUrl: `https://${id}.example`,
  unread: 0,
  lastError,
  lastSuccessAt: null,
  favicon: null,
  ...extra,
});

const brokenSidebar = (): Sidebar => ({
  allUnread: 0,
  starred: 0,
  today: 0,
  folders: [{ id: 2, name: 'Tech', unread: 0, feeds: [feed(5, 'Rust Blog', 'HTTP 404'), feed(6, 'Fine Feed', null)] }],
  feeds: [feed(7, 'Old News', 'timed out', { lastSuccessAt: 1_700_000_000 })],
});

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 0),
  list: vi.fn(async (_q: ListQuery, _o: number, _l: number) => []),
  refresh: vi.fn(async () => true),
  sidebar: vi.fn(async (): Promise<Sidebar> => brokenSidebar()),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false })),
  retryFeed: vi.fn(async (_id: number) => ({ error: null as string | null, newArticles: 0 })),
  unsubscribeFeeds: vi.fn(async (ids: number[]) => ids.length),
  openExternal: vi.fn(async (_url: string) => {}),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

const { app } = await import('../state.svelte');
const { default: App } = await import('../../App.svelte');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const dialog = () => document.querySelector('.broken-feeds-dialog');
const rows = () => [...document.querySelectorAll('.broken-feeds-dialog [role="listitem"]')];
const button = (name: string) =>
  [...document.querySelectorAll<HTMLButtonElement>('.broken-feeds-dialog button')].find(
    (b) => b.getAttribute('aria-label') === name || b.textContent?.trim() === name,
  );

let mounted: ReturnType<typeof mount> | null = null;

async function mountApp() {
  mounted = mount(App, { target: document.body });
  await settle();
  app.phase = 'ready';
  app.sidebar = brokenSidebar();
  flushSync();
}

async function openDialog() {
  app.brokenFeedsOpen = true;
  flushSync();
  await settle();
  flushSync();
  expect(dialog(), 'dialog is open').not.toBeNull();
}

beforeEach(() => {
  vi.clearAllMocks();
  app.brokenFeedsOpen = false;
  app.selection = { kind: 'allUnread' };
  app.selectedIndex = null;
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  app.brokenFeedsOpen = false;
  document.body.innerHTML = '';
});

describe('Broken Feeds dialog', () => {
  it('is offered in the command palette only while there are broken Feeds', async () => {
    await mountApp();
    const paletteHas = async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true }));
      flushSync();
      await settle();
      flushSync();
      const found = document.body.textContent?.includes('Broken Feeds (2)…');
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true }));
      flushSync();
      await settle();
      return found;
    };
    expect(await paletteHas()).toBe(true);
    app.sidebar = { ...brokenSidebar(), folders: [], feeds: [feed(6, 'Fine Feed', null)] };
    flushSync();
    expect(await paletteHas()).toBe(false);
  });

  it('lists each broken Feed with its Folder, last error and last success', async () => {
    await mountApp();
    await openDialog();
    const text = rows().map((r) => r.textContent?.replace(/\s+/g, ' ').trim());
    expect(text).toHaveLength(2);
    expect(text[0]).toContain('Rust Blog');
    expect(text[0]).toContain('Tech');
    expect(text[0]).toContain('HTTP 404');
    expect(text[0]).toContain('has never worked');
    expect(text[1]).toContain('Old News');
    expect(text[1]).toContain('timed out');
    expect(text[1]).toContain('last worked');
    expect(dialog()!.textContent).not.toContain('Fine Feed');
  });

  it('retries one Feed now and drops it from the list once it works', async () => {
    await mountApp();
    await openDialog();
    mockApi.sidebar.mockResolvedValueOnce({
      ...brokenSidebar(),
      folders: [{ id: 2, name: 'Tech', unread: 0, feeds: [feed(5, 'Rust Blog', null), feed(6, 'Fine Feed', null)] }],
    });

    button('Retry Rust Blog now')!.click();
    await settle();
    await settle();
    flushSync();

    expect(mockApi.retryFeed).toHaveBeenCalledWith(5);
    expect(rows().map((r) => r.textContent)).toEqual([expect.stringContaining('Old News')]);
    expect(dialog()!.querySelector('[role="status"]')?.textContent).toBe('Rust Blog works again');
  });

  it('shows the new error when a retry fails again', async () => {
    await mountApp();
    await openDialog();
    mockApi.retryFeed.mockResolvedValueOnce({ error: 'HTTP 410', newArticles: 0 });
    mockApi.sidebar.mockResolvedValueOnce({
      ...brokenSidebar(),
      folders: [{ id: 2, name: 'Tech', unread: 0, feeds: [feed(5, 'Rust Blog', 'HTTP 410')] }],
    });

    button('Retry Rust Blog now')!.click();
    await settle();
    await settle();
    flushSync();

    expect(rows()[0]!.textContent).toContain('HTTP 410');
    expect(dialog()!.querySelector('[role="status"]')?.textContent).toBe('Rust Blog still fails: HTTP 410');
  });

  it('asks before unsubscribing from all listed Feeds and names the count', async () => {
    await mountApp();
    await openDialog();
    button('Unsubscribe from all 2 Feeds…')!.click();
    flushSync();

    expect(dialog()!.textContent!.replace(/\s+/g, ' ')).toContain(
      'Unsubscribe from 2 Feeds? Their Articles and cached images are removed',
    );
    expect(mockApi.unsubscribeFeeds).not.toHaveBeenCalled();
    const actions = [...dialog()!.querySelectorAll('.confirm button')].map((b) => b.textContent?.trim());
    expect(actions).toEqual(['Cancel', 'Unsubscribe from 2 Feeds']);
    expect(document.activeElement?.textContent?.trim(), 'Cancel takes focus').toBe('Cancel');

    button('Unsubscribe from 2 Feeds')!.click();
    await settle();
    expect(mockApi.unsubscribeFeeds).toHaveBeenCalledWith([5, 7]);
  });

  it('asks before unsubscribing from one Feed, and Cancel keeps it', async () => {
    await mountApp();
    await openDialog();
    button('Unsubscribe from Rust Blog')!.click();
    flushSync();
    expect(dialog()!.textContent).toContain('Unsubscribe from Rust Blog?');
    button('Cancel')!.click();
    flushSync();
    expect(mockApi.unsubscribeFeeds).not.toHaveBeenCalled();
    expect(dialog()!.textContent).not.toContain('Unsubscribe from Rust Blog?');
  });

  it('renders only a window of rows when there are hundreds of broken Feeds', async () => {
    await mountApp();
    const many = Array.from({ length: 357 }, (_, i) => feed(100 + i, `Feed ${i}`, 'HTTP 500'));
    app.sidebar = { allUnread: 0, starred: 0, today: 0, folders: [], feeds: many };
    flushSync();
    await openDialog();
    expect(rows().length).toBeGreaterThan(0);
    expect(rows().length).toBeLessThan(40);
    expect(rows()[0]!.getAttribute('aria-setsize')).toBe('357');
    expect(dialog()!.textContent).toContain('357 Feeds failed');
  });
});
