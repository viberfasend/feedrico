// @vitest-environment happy-dom
// The View-level Assistant surface (#120 briefing, #121 picks): the Assistant menu in the
// Article list header, the panel above the list, links that open Articles like a list click,
// the picks highlight in the rows, and that nothing reorders or marks anything by itself.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ArticleDetail, ArticleRow, ListQuery, Settings, Sidebar } from '../api';
import type { AssistantEvent, AssistantResult, AssistantSetup } from '../assistant';

const defaultSettings = (): Settings => ({
  appearance: { theme: 'system', density: 'comfortable' },
  reader: { fontFamily: 'serif', fontSize: 18, measure: 66, lineHeight: 1.65 },
  refresh: { intervalMinutes: 30 },
  imageCacheCapBytes: 1024 ** 3,
  pruneReadDays: 90,
  pruneUnreadDays: 365,
});

const sidebar: Sidebar = {
  allUnread: 2,
  starred: 0,
  today: 0,
  folders: [],
  feeds: [{ id: 5, title: 'Rust Blog', siteUrl: null, unread: 2, lastError: null, lastSuccessAt: null, favicon: null }],
};

const row = (i: number): ArticleRow => ({
  id: i + 1,
  feedId: 5,
  feedTitle: 'Rust Blog',
  title: `Article ${i + 1}`,
  excerpt: `excerpt ${i + 1}`,
  articleDate: 1_700_000_000 - i,
  isRead: false,
  isStarred: false,
  leadImage: null,
  favicon: null,
});

const mockApi = {
  count: vi.fn(async (_q: ListQuery) => 2),
  list: vi.fn(async (_q: ListQuery, offset: number, limit: number) =>
    Array.from({ length: Math.max(0, Math.min(limit, 2 - offset)) }, (_, i) => row(offset + i)),
  ),
  articleIndex: vi.fn(async (_q: ListQuery, id: number) => (id === 1 ? 0 : id === 2 ? 1 : null)),
  article: vi.fn(async (id: number): Promise<ArticleDetail> => ({
    ...row(id - 1),
    feedSiteUrl: null,
    url: `https://example.com/${id}`,
    author: null,
    contentHtml: '<p>Hello</p>',
    enclosures: [],
  })),
  setRead: vi.fn(async () => {}),
  setLastView: vi.fn(async () => {}),
  sidebar: vi.fn(async (): Promise<Sidebar> => sidebar),
  openExternal: vi.fn(async () => {}),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

interface FakeTask {
  action: unknown;
  emit: (e: AssistantEvent) => void;
  resolve: (r: AssistantResult) => void;
  reject: (e: unknown) => void;
  cancel: ReturnType<typeof vi.fn>;
}
const tasks: FakeTask[] = [];
let setup: AssistantSetup | null = null;

vi.mock('../assistant', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../assistant')>()),
  getAssistantSetup: vi.fn(async () => setup),
  runAssistant: vi.fn((action: unknown, onEvent: (e: AssistantEvent) => void) => {
    let resolve!: FakeTask['resolve'];
    let reject!: FakeTask['reject'];
    const result = new Promise<AssistantResult>((res, rej) => ((resolve = res), (reject = rej)));
    result.catch(() => {});
    const cancel = vi.fn(async () => reject({ kind: 'cancelled', message: 'Cancelled' }));
    tasks.push({ action, emit: onEvent, resolve, reject, cancel });
    return { runId: `r${tasks.length}`, result, cancel };
  }),
}));

const { app } = await import('../state.svelte');
const { assist } = await import('../assistant-state.svelte');
const { default: ArticleList } = await import('./ArticleList.svelte');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
let mounted: ReturnType<typeof mount> | null = null;
const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const press = (key: string) => {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
  flushSync();
};

const SETUP: AssistantSetup = {
  kind: 'claudeCode',
  path: '/bin/claude',
  model: null,
  disclosureAccepted: true,
  interests: 'Rust, local-first software; not crypto',
};

async function renderList(withSetup: AssistantSetup | null = SETUP) {
  setup = withSetup;
  assist.setup = withSetup;
  app.phase = 'ready';
  app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false };
  app.sidebar = sidebar;
  app.selection = { kind: 'allUnread' };
  app.search = '';
  app.sort = 'newestFirst';
  app.layouts = { default: 'list', views: {} };
  app.settings = defaultSettings();
  mounted = mount(ArticleList, { target: document.body });
  flushSync();
  await app.resetList();
  await settle();
  flushSync();
}

const briefingEntry = () => assist.viewEntries.find((e) => e.id === 'briefing')!;
const picksEntry = () => assist.viewEntries.find((e) => e.id === 'picks')!;

const briefingResult: AssistantResult = {
  text: '',
  structured: {
    considered: 2,
    capped: false,
    topics: [{ topic: 'Rust', sentence: 'Two posts about Rust.', articleIds: [1, 2] }],
    articles: [
      { id: 1, title: 'Article 1', feedTitle: 'Rust Blog' },
      { id: 2, title: 'Article 2', feedTitle: 'Rust Blog' },
    ],
  },
};

const picksResult: AssistantResult = {
  text: '2 Articles considered',
  structured: {
    considered: 2,
    capped: false,
    picks: [{ id: 1, title: 'Article 1', feedTitle: 'Rust Blog', score: 0.9, reason: 'right on topic' }],
  },
};

beforeEach(() => {
  vi.clearAllMocks();
  tasks.length = 0;
  mockApi.count.mockResolvedValue(2);
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  assist.reset();
  app.article = null;
  app.selectedIndex = null;
  app.readerOpen = false;
  document.body.innerHTML = '';
});

describe('the View actions in the View menu', () => {
  async function viewMenuLabels() {
    const trigger = q('#pane-list button[aria-label^="View options"]') as HTMLButtonElement;
    trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    flushSync();
    await settle();
    return [...document.querySelectorAll<HTMLElement>('[role="menuitem"], [role="menuitemradio"]')].map((el) => el.textContent?.trim());
  }

  it('are absent without a setup, and nothing runs', async () => {
    await renderList(null);
    expect(await viewMenuLabels()).not.toContain('Brief this View');
    assist.start(briefingEntry());
    flushSync();
    expect(tasks).toHaveLength(0);
  });

  it('add no button of their own to the list header (the search field keeps its room)', async () => {
    await renderList();
    expect(q('#pane-list button[aria-label="Assistant"]')).toBeNull();
  });

  it('start a briefing with the current query from the View menu', async () => {
    await renderList();
    const labels = await viewMenuLabels();
    expect(labels).toContain('Brief this View');
    expect(labels).toContain('Picks for this View');
    const item = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      (el) => el.textContent?.trim() === 'Brief this View',
    )!;
    item.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    flushSync();
    await settle();
    expect(tasks).toHaveLength(1);
    const action = tasks[0]!.action as { kind: string; query: ListQuery };
    expect(action.kind).toBe('briefing');
    expect(action.query.view).toEqual({ kind: 'allUnread' });
    expect(action.query.sort).toBe('newestFirst');
  });
});

describe('the briefing panel (#120)', () => {
  it('shows the labelled basis, the topics, and links that open Articles like a list click', async () => {
    await renderList();
    assist.start(briefingEntry());
    flushSync();
    await settle();
    expect(q('.vpanel')).not.toBeNull();
    expect(q('.vpanel')!.textContent).toContain('Written by Claude Code · may be wrong');
    expect(q('.vpanel .stream')!.textContent).toBe('Working…');
    tasks.at(-1)!.resolve(briefingResult);
    await settle();
    flushSync();
    await settle();
    flushSync();
    const panel = q('.vpanel')!;
    expect(panel.textContent).toContain('Based on all 2 Articles');
    expect(panel.textContent).toContain('Rust.');
    expect(panel.textContent).toContain('Two posts about Rust.');
    const links = [...panel.querySelectorAll('button.alink')];
    expect(links.map((l) => l.textContent)).toEqual(['Article 1', 'Article 2']);
    // A link opens the Article exactly like a list click: index lookup, reader, marked Read.
    (links[0] as HTMLButtonElement).click();
    await settle();
    flushSync();
    expect(mockApi.articleIndex).toHaveBeenCalledWith(expect.objectContaining({ view: { kind: 'allUnread' } }), 1);
    expect(mockApi.article).toHaveBeenCalledWith(1);
    expect(mockApi.setRead).toHaveBeenCalledWith(1, true);
    expect(app.selectedIndex).toBe(0);
    expect(app.article?.id).toBe(1);
    expect(q('[aria-live="polite"]')!.textContent).toBe('Answer ready');
  });

  it('renders an Article that is not in the View as plain text, not a dead link', async () => {
    await renderList();
    assist.start(briefingEntry());
    flushSync();
    tasks.at(-1)!.resolve({
      text: '',
      structured: {
        considered: 2,
        capped: true,
        topics: [{ topic: 'T', sentence: 's', articleIds: [7] }],
        articles: [{ id: 7, title: 'Elsewhere', feedTitle: 'Other Feed' }],
      },
    });
    await settle();
    flushSync();
    await settle();
    flushSync();
    const panel = q('.vpanel')!;
    expect(panel.textContent).toContain('Based on the newest 2 Articles');
    expect(panel.querySelector('button.alink')).toBeNull();
    expect(panel.querySelector('.alink.dead')!.textContent).toBe('Elsewhere');
  });

  it('Esc stops a running briefing and keeps the panel', async () => {
    await renderList();
    assist.start(briefingEntry());
    flushSync();
    await settle();
    press('Escape');
    await settle();
    flushSync();
    expect(tasks.at(-1)!.cancel).toHaveBeenCalledTimes(1);
    expect(assist.viewRun!.phase).toBe('stopped');
    expect(q('.vpanel')).not.toBeNull();
  });

  it('changing the View discards the run', async () => {
    await renderList();
    assist.start(briefingEntry());
    flushSync();
    tasks.at(-1)!.resolve(briefingResult);
    await settle();
    flushSync();
    expect(assist.viewRun).not.toBeNull();
    await app.select({ kind: 'starred' });
    await settle();
    flushSync();
    expect(assist.viewRun).toBeNull();
    expect(q('.vpanel')).toBeNull();
  });

  it('shows plain words for errors, including missing interests', async () => {
    await renderList();
    assist.start(briefingEntry());
    flushSync();
    tasks.at(-1)!.reject({ kind: 'noInterests', message: 'x' });
    await settle();
    flushSync();
    expect(q('.vpanel .error')!.textContent).toContain('Picks need to know what you like to read');
    assist.start(picksEntry());
    flushSync();
    tasks.at(-1)!.reject({ kind: 'invalid', message: 'no Articles in this View' });
    await settle();
    flushSync();
    expect(q('.vpanel .error')!.textContent).toContain('did not work for this View');
  });
});

describe('picks (#121)', () => {
  it('shows how many were considered, each pick with its reason, and highlights the rows', async () => {
    await renderList();
    assist.start(picksEntry());
    flushSync();
    const action = tasks.at(-1)!.action as { kind: string; query: ListQuery };
    expect(action.kind).toBe('picks');
    tasks.at(-1)!.resolve(picksResult);
    await settle();
    flushSync();
    await settle();
    flushSync();
    const panel = q('.vpanel')!;
    expect(panel.textContent).toContain('Ranked all 2 Articles of this View (unread)');
    expect(panel.textContent).toContain('right on topic');
    expect(panel.textContent).toContain('90');
    expect([...panel.querySelectorAll('button.alink')].map((b) => b.textContent)).toEqual(['Article 1']);
    // The session highlight: the picked row carries a Pick badge with the reason as its tooltip.
    expect(assist.picks).not.toBeNull();
    const badge = q('#pane-list .row .pick');
    expect(badge?.textContent).toBe('Pick');
    expect(badge?.getAttribute('title')).toBe('right on topic');
    // The chronological sort is untouched: still newest first, and the list was never re-queried.
    expect(app.sort).toBe('newestFirst');
    const listCalls = mockApi.list.mock.calls.map((c) => (c[0] as ListQuery).sort);
    expect(listCalls).toEqual(listCalls.map(() => 'newestFirst'));
  });

  it('clears the highlight when the View changes', async () => {
    await renderList();
    assist.start(picksEntry());
    flushSync();
    tasks.at(-1)!.resolve(picksResult);
    await settle();
    flushSync();
    await settle();
    expect(q('#pane-list .row .pick')).not.toBeNull();
    await app.select({ kind: 'starred' });
    await settle();
    flushSync();
    expect(assist.picks).toBeNull();
    expect(q('#pane-list .row .pick')).toBeNull();
  });
});
