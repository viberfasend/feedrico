// @vitest-environment happy-dom
// Automated WCAG check (issue #32): axe-core runs over the mounted app in every Layout, with the
// slide-over reader open, the Settings dialog open and the command palette open. Text contrast is
// disabled (no real layout in a DOM emulator; the token build covers contrast in both themes) —
// everything else must be clean.
import axeModule from 'axe-core';

// axe-core is CJS: the default export is the axe object; the async API is axe.run(context, options).
interface AxeResults {
  violations: { id: string; impact: string; nodes: { target: string[] }[] }[];
}
const axe = axeModule as unknown as {
  run: (context: Element, options: object) => Promise<AxeResults>;
};
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ArticleDetail, ArticleRow, Layout, ListQuery, Settings, Sidebar } from '../api';

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
    excerpt: 'An excerpt',
    articleDate: 1_700_000_000 - i,
    isRead: i % 2 === 1,
    isStarred: i === 1,
    leadImage: null,
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
      feeds: [{ id: 5, title: 'Rust Blog', siteUrl: 'https://blog.rust-lang.org', unread: 2, lastError: 'HTTP 404', lastSuccessAt: 1_700_000_000 }],
    },
  ],
  feeds: [{ id: 7, title: 'Hacker News', siteUrl: 'https://news.yorker.com', unread: 1, lastError: null, lastSuccessAt: null }],
};

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 4),
  list: vi.fn(async (_q: ListQuery, offset: number, limit: number) =>
    Array.from({ length: Math.max(0, Math.min(limit, 4 - offset)) }, (_, i) => row(offset + i)),
  ),
  refresh: vi.fn(async () => true),
  sidebar: vi.fn(async (): Promise<Sidebar> => sidebar),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 2, portable: false })),
  layouts: vi.fn(async () => ({ default: 'list', views: {} })),
  setLayout: vi.fn(async () => {}),
  article: vi.fn(
    async (id: number): Promise<ArticleDetail> => ({
      ...row(id - 1),
      feedSiteUrl: null,
      url: 'https://example.com/a',
      author: null,
      contentHtml: '<p>Hello</p>',
      enclosures: [],
    }),
  ),
  setRead: vi.fn(async () => {}),
  deleteFolder: vi.fn(async () => {}),
  openExternal: vi.fn(async () => {}),
  checkForUpdates: vi.fn(),
  settings: vi.fn(async () => defaultSettings()),
  setSettings: vi.fn(async () => {}),
  hasSetting: vi.fn(async () => false),
  librarySize: vi.fn(async () => 1000),
  mediaSize: vi.fn(async () => 500),
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

let mounted: ReturnType<typeof mount> | null = null;

async function mountReady(layout: Layout) {
  mounted = mount(App, { target: document.body });
  await settle();
  app.phase = 'ready';
  app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 2, portable: false };
  app.settings = defaultSettings();
  app.sidebar = sidebar;
  app.layouts = { default: layout, views: {} };
  flushSync();
  await app.resetList();
  await settle();
  flushSync();
}

async function expectNoViolations(label: string) {
  const results = await axe.run(document.body, {
    runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] },
    rules: { 'color-contrast': { enabled: false } },
  });
  const violations = results.violations.map(
    (v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`,
  );
  // Guard against a vacuous run: axe must have evaluated a real rule set.
  expect((results as unknown as { passes: unknown[] }).passes.length, `${label}: axe ran rules`).toBeGreaterThan(10);
  expect(violations, `${label} must have no axe violations`).toEqual([]);
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

describe('axe over every screen state', () => {
  for (const layout of ['list', 'cards', 'magazine', 'gallery'] as Layout[]) {
    it(`has no violations in the ${layout} Layout`, async () => {
      await mountReady(layout);
      await expectNoViolations(layout);
    });
  }

  it('has no violations with the slide-over reader open', async () => {
    await mountReady('cards');
    await app.selectIndex(0);
    await settle();
    flushSync();
    expect(app.readerOpen).toBe(true);
    await expectNoViolations('slide-over reader');
  });

  it('has no violations with the Settings dialog open', async () => {
    await mountReady('list');
    await app.openSettings();
    await settle();
    flushSync();
    expect(document.querySelector('.settings-dialog')).not.toBeNull();
    await expectNoViolations('settings dialog');
  });

  it('has no violations with the command palette open', async () => {
    await mountReady('list');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true, cancelable: true }));
    await settle();
    flushSync();
    expect(document.querySelector('[role="combobox"]')).not.toBeNull();
    await expectNoViolations('command palette');
  });
});
