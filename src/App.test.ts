// @vitest-environment happy-dom
// Keyboard-first reading (spec "Keyboard"; Henrik, The Archivist: read 50 Articles with
// j/k/s/m only, switch Layouts with v, page with Space). Shortcuts never fire while typing.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ArticleDetail, ArticleRow, ListQuery, Settings, Sidebar } from './lib/api';

const defaultSettings = (): Settings => ({
  appearance: { theme: 'system', density: 'comfortable' },
  reader: { fontFamily: 'serif', fontSize: 18, measure: 66, lineHeight: 1.65 },
  refresh: { intervalMinutes: 30 },
  imageCacheCapBytes: 1024 ** 3,
  pruneReadDays: 90,
  pruneUnreadDays: 365,
});

function row(i: number): ArticleRow {
  return {
    id: i + 1,
    feedId: 1,
    feedTitle: 'Rust Blog',
    title: `Article ${i + 1}`,
    excerpt: 'excerpt',
    articleDate: 1_700_000_000 - i,
    isRead: false,
    isStarred: false,
    leadImage: null,
    favicon: null,
  };
}

const sidebar: Sidebar = {
  allUnread: 3,
  starred: 0,
  today: 0,
  folders: [],
  feeds: [{ id: 1, title: 'Rust Blog', siteUrl: null, unread: 3, lastError: null, lastSuccessAt: null, favicon: null }],
};

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 3),
  list: vi.fn(async (_q: ListQuery, offset: number, limit: number) =>
    Array.from({ length: Math.min(limit, 3 - offset) }, (_, i) => row(offset + i)),
  ),
  refresh: vi.fn(async () => true),
  sidebar: vi.fn(async (): Promise<Sidebar> => sidebar),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false })),
  layouts: vi.fn(async () => ({ default: 'list', views: {} })),
  setLayout: vi.fn(async () => {}),
  lastView: vi.fn(async () => null as string | null),
  setLastView: vi.fn(async () => {}),
  article: vi.fn(async (id: number): Promise<ArticleDetail> => ({
    ...row(id - 1),
    feedSiteUrl: null,
    url: `https://example.com/a/${id}`,
    author: null,
    contentHtml: '<p>hi</p>',
    enclosures: [],
  })),
  setRead: vi.fn(async () => {}),
  setStarred: vi.fn(async () => {}),
  markAllRead: vi.fn(async () => [1, 2, 3] as number[]),
  markUnread: vi.fn(async () => {}),
  deleteFolder: vi.fn(async () => {}),
  openExternal: vi.fn(async () => {}),
  checkForUpdates: vi.fn(),
  settings: vi.fn(async () => defaultSettings()),
  setSettings: vi.fn(async () => {}),
  hasSetting: vi.fn(async () => false),
  librarySize: vi.fn(async () => 0),
  mediaSize: vi.fn(async () => 0),
  autostartEnabled: vi.fn(async () => false),
  setAutostart: vi.fn(async () => {}),
};

vi.mock('./lib/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./lib/api')>()),
  api: mockApi,
}));

// happy-dom's Animation has no cancel(); zero-duration transitions skip it entirely.
vi.mock('svelte/transition', () => ({
  fade: () => ({ duration: 0 }),
  fly: () => ({ duration: 0 }),
}));

const { app } = await import('./lib/state.svelte');
const { default: App } = await import('./App.svelte');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function press(key: string, init: KeyboardEventInit = {}, target: EventTarget = window) {
  target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
  flushSync();
}

let mounted: ReturnType<typeof mount> | null = null;

async function mountReady(total = 3) {
  mockApi.count.mockResolvedValue(total);
  mounted = mount(App, { target: document.body });
  await settle();
  app.phase = 'ready';
  app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false };
  app.settings = defaultSettings();
  app.sidebar = sidebar;
  app.layouts = { default: 'list', views: {} };
  flushSync();
  await app.resetList();
  await settle();
  flushSync();
}

beforeEach(() => {
  vi.clearAllMocks();
  mockApi.count.mockResolvedValue(3);
  localStorage.clear();
  app.settingsOpen = false;
  app.addFeedOpen = false;
  app.selectedIndex = null;
  app.article = null;
  app.readerOpen = false;
  app.toasts = [];
  app.hasNewArticles = false;
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

describe('keyboard reading (j/k/s/m/o/r)', () => {
  it('j opens the first Article and marks it Read; j/k move through the list', async () => {
    await mountReady();
    press('j');
    await settle();
    expect(mockApi.article).toHaveBeenCalledWith(1);
    expect(app.selectedIndex).toBe(0);
    expect(app.article?.id).toBe(1);
    // Opening an Article marks it Read (spec: "Read state").
    expect(mockApi.setRead).toHaveBeenCalledWith(1, true);

    press('j');
    await settle();
    expect(app.selectedIndex).toBe(1);
    expect(app.article?.id).toBe(2);
    press('k');
    await settle();
    expect(app.selectedIndex).toBe(0);
    expect(app.article?.id).toBe(1);
  });

  it('s toggles Starred and m toggles Read on the open Article', async () => {
    await mountReady();
    press('j');
    await settle();
    press('s');
    await settle();
    expect(mockApi.setStarred).toHaveBeenCalledWith(1, true);
    expect(app.article?.isStarred).toBe(true);

    press('m');
    await settle();
    expect(mockApi.setRead).toHaveBeenCalledWith(1, false);
    expect(app.article?.isRead).toBe(false);
  });

  it('o opens the Article in the system browser, r refreshes', async () => {
    await mountReady();
    press('j');
    await settle();
    press('o');
    await settle();
    expect(mockApi.openExternal).toHaveBeenCalledWith('https://example.com/a/1');
    press('r');
    await settle();
    expect(mockApi.refresh).toHaveBeenCalledWith(true);
  });

  it('v cycles through the Layouts and persists the choice for the View', async () => {
    await mountReady();
    expect(app.layout).toBe('list');
    press('v');
    await settle();
    expect(app.layout).toBe('cards');
    expect(mockApi.setLayout).toHaveBeenCalledWith('allUnread', 'cards');
    press('v');
    await settle();
    expect(app.layout).toBe('magazine');
  });

  it('arrow keys move spatially in grid Layouts', async () => {
    await mountReady(10);
    app.layouts = { default: 'cards', views: {} };
    flushSync();
    await app.resetList();
    await settle();
    // The grid reports its column count from its width; happy-dom's width is 0, so the Layout
    // would report 1 column — set it after the Layout settled.
    app.gridColumns = 3;
    flushSync();
    expect(app.layout).toBe('cards');
    press('ArrowDown'); // nothing selected yet: the first move lands on row 0
    await settle();
    expect(app.selectedIndex).toBe(0);
    press('ArrowDown'); // one grid row down: +3 columns
    await settle();
    expect(app.selectedIndex).toBe(3);
    press('ArrowRight');
    await settle();
    expect(app.selectedIndex).toBe(4);
    // In a grid, moving the highlight does not open (and thus not mark Read) the Article.
    expect(mockApi.article).not.toHaveBeenCalled();
  });

  it('Enter opens the slide-over reader in a grid Layout, Escape closes it', async () => {
    await mountReady();
    app.layouts = { default: 'cards', views: {} };
    flushSync();
    await app.resetList();
    await settle();
    press('ArrowDown'); // highlight without opening
    await settle();
    expect(app.readerOpen).toBe(false);
    press('Enter');
    await settle();
    expect(app.readerOpen).toBe(true);
    expect(document.querySelector('.slide-over [role="dialog"], .slide-over')).not.toBeNull();
    press('Escape');
    flushSync();
    expect(app.readerOpen).toBe(false);
  });

  it('Space at the end of an Article moves to the next one', async () => {
    await mountReady();
    press('j');
    await settle();
    expect(app.selectedIndex).toBe(0);
    // happy-dom reports a 0 px reader pane: already at the bottom, so Space pages forward.
    press(' ');
    await settle();
    expect(app.selectedIndex).toBe(1);
  });

  it('shortcuts do not fire while typing in the search field', async () => {
    await mountReady();
    const input = document.getElementById('search') as HTMLInputElement;
    input.focus();
    press('j', {}, input);
    await settle();
    expect(app.selectedIndex).toBeNull();
    expect(mockApi.article).not.toHaveBeenCalled();
  });

  it('a opens the Add Feed dialog', async () => {
    await mountReady();
    press('a');
    flushSync();
    expect(app.addFeedOpen).toBe(true);
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
  });
});

describe('mark all as read with undo (Henrik)', () => {
  it('marks the whole View read and Undo restores it', async () => {
    await mountReady();
    const btn = [...document.querySelectorAll<HTMLButtonElement>('#pane-list button')].find(
      (b) => b.getAttribute('aria-label') === 'Mark all as read',
    )!;
    btn.click();
    await settle();
    flushSync();
    expect(mockApi.markAllRead).toHaveBeenCalledWith(app.query());
    expect(app.rowAt(0)?.isRead).toBe(true);
    expect(app.rowAt(2)?.isRead).toBe(true);

    const toast = document.querySelector('.toast')!;
    expect(toast.textContent).toContain('3 marked as read');
    const undo = [...toast.querySelectorAll('button')].find((b) => b.textContent === 'Undo')!;
    undo.click();
    await settle();
    expect(mockApi.markUnread).toHaveBeenCalledWith([1, 2, 3]);
    expect(app.rowAt(0)?.isRead).toBe(false);
  });
});
