// @vitest-environment happy-dom
// Accessibility regression tests (issue #32): virtualized rows announce their position
// ("5 of 1,240"), unread counts and row state are announced in words, F6/Shift+F6 cycle the
// panes, and Esc in the slide-over reader returns focus to the Article row it came from.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ArticleDetail, ArticleRow, ListQuery, Sidebar } from '../api';

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
  article: vi.fn(async (id: number): Promise<ArticleDetail> => ({ ...row(id - 1), feedSiteUrl: null, url: 'https://example.com/a', author: null, contentHtml: '<p>hi</p>', enclosures: [] })),
  setRead: vi.fn(async () => {}),
  deleteFolder: vi.fn(async () => {}),
  openExternal: vi.fn(async () => {}),
  checkForUpdates: vi.fn(),
  settings: vi.fn(async () => ({
    appearance: { theme: 'system', density: 'comfortable' },
    reader: { fontFamily: 'serif', fontSize: 18, measure: 66, lineHeight: 1.65 },
    refresh: { intervalMinutes: 30 },
    imageCacheCapBytes: 1024 ** 3,
    pruneReadDays: 90,
    pruneUnreadDays: 365,
  })),
  setSettings: vi.fn(async () => {}),
  hasSetting: vi.fn(async () => false),
  librarySize: vi.fn(async () => 0),
  mediaSize: vi.fn(async () => 0),
  autostartEnabled: vi.fn(async () => false),
  setAutostart: vi.fn(async () => {}),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

// happy-dom's Animation has no cancel(); zero-duration transitions skip it entirely.
vi.mock('svelte/transition', () => ({
  fade: () => ({ duration: 0 }),
  fly: () => ({ duration: 0 }),
}));

const { app } = await import('../state.svelte');
const { default: App } = await import('../../App.svelte');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function press(key: string, init: KeyboardEventInit = {}) {
  document.body.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
  flushSync();
}

function row(i: number): ArticleRow {
  return {
    id: i + 1,
    feedId: 1,
    feedTitle: 'Rust Blog',
    title: `Article ${i + 1}`,
    excerpt: 'excerpt',
    articleDate: 1_700_000_000 - i,
    isRead: false,
    isStarred: i === 1,
    leadImage: null,
    favicon: null,
  };
}

const sidebar: Sidebar = {
  allUnread: 3,
  starred: 1,
  today: 1,
  folders: [
    {
      id: 2,
      name: 'Tech',
      unread: 2,
      feeds: [{ id: 5, title: 'Rust Blog', siteUrl: null, unread: 2, lastError: 'HTTP 404', lastSuccessAt: null, favicon: null }],
    },
  ],
  feeds: [],
};

let mounted: ReturnType<typeof mount> | null = null;

async function mountReady() {
  mounted = mount(App, { target: document.body });
  await settle();
  app.phase = 'ready';
  app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false };
  app.settings = (await mockApi.settings()) as never;
  app.sidebar = sidebar;
  app.layouts = { default: 'list', views: {} };
  flushSync();
  await settle();
  await settle();
  flushSync();
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  app.settingsOpen = false;
  app.addFeedOpen = false;
  app.selectedIndex = null;
  app.article = null;
  app.readerOpen = false;
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

describe('virtualized rows announce position and state', () => {
  it('rendered options carry aria-setsize and aria-posinset ("2 of 3")', async () => {
    await mountReady();
    app.total = 3;
    await app.resetList();
    await settle();
    flushSync();
    const options = [...document.querySelectorAll('#pane-list [role="option"]')] as HTMLElement[];
    expect(options.length).toBeGreaterThan(0);
    expect(options[0]!.getAttribute('aria-setsize')).toBe('3');
    expect(options[0]!.getAttribute('aria-posinset')).toBe('1');
    expect(options[1]!.getAttribute('aria-posinset')).toBe('2');
  });

  it('announces Read and Starred state in the option label', async () => {
    await mountReady();
    await app.resetList();
    await settle();
    flushSync();
    const options = [...document.querySelectorAll('#pane-list [role="option"]')] as HTMLElement[];
    expect(options[0]!.getAttribute('aria-label')).toContain('unread');
    expect(options[1]!.getAttribute('aria-label')).toContain('starred');
  });

  it('unread counts read as "N unread", not a bare number', async () => {
    await mountReady();
    const counts = [...document.querySelectorAll('#pane-sidebar .count .sr-only')].map(
      (el) => el.textContent,
    );
    expect(counts).toContain('3 unread');
    expect(counts).toContain('2 unread');
  });

  it('the broken-Feed badge has a text alternative', async () => {
    await mountReady();
    const badge = document.querySelector('#pane-sidebar [role="img"]');
    expect(badge?.getAttribute('aria-label')).toBe('Feed has problems');
  });
});

describe('pane landmarks and F6', () => {
  it('has one main landmark and labelled panes', async () => {
    await mountReady();
    expect(document.querySelectorAll('main').length).toBe(1);
    expect(document.getElementById('pane-sidebar')?.getAttribute('aria-label')).toBe('Feeds');
    expect(document.getElementById('pane-list')?.getAttribute('aria-label')).toBe('Articles');
    expect(document.getElementById('pane-reader')?.getAttribute('aria-label')).toBe('Reader');
  });

  it('F6 moves focus sidebar → list → reader and Shift+F6 goes back', async () => {
    await mountReady();
    (document.getElementById('pane-sidebar') as HTMLElement).focus();
    expect(document.activeElement?.id).toBe('pane-sidebar');
    press('F6');
    expect(document.activeElement?.id).toBe('pane-list');
    press('F6');
    expect(document.activeElement?.id).toBe('pane-reader');
    press('F6', { shiftKey: true });
    expect(document.activeElement?.id).toBe('pane-list');
  });
});

describe('slide-over reader focus', () => {
  it('moves focus into the reader on open and returns it to the row on Esc', async () => {
    await mountReady();
    app.layouts = { default: 'cards', views: {} };
    flushSync();
    await app.resetList();
    await settle();
    flushSync();
    await app.selectIndex(0);
    await settle();
    flushSync();
    expect(app.readerOpen).toBe(true);
    const slideOver = document.querySelector('.slide-over') as HTMLElement;
    expect(slideOver).not.toBeNull();
    expect(slideOver.contains(document.activeElement), 'focus moved into the reader').toBe(true);

    press('Escape');
    await settle();
    flushSync();
    await settle();
    flushSync();
    expect(app.readerOpen).toBe(false);
    const selected = document.querySelector('#pane-list [role="option"][aria-selected="true"]');
    expect(selected, 'the selected row exists').not.toBeNull();
    expect(document.activeElement === selected, 'focus returned to the row the reader came from').toBe(true);
  });
});
