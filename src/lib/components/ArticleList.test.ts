// @vitest-environment happy-dom
// The Article list pane: View title and count, the View menu (Layout and sort order), debounced
// search with its Folder/Feed scope chip, the empty states (Mira), the refresh progress bar
// and the "New Articles" pill. Clara: every View remembers its own Layout.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ArticleRow, ListQuery, Settings, Sidebar } from '../api';

const dialogOpen = vi.fn(async (_opts: unknown) => null as string | null);

const defaultSettings = (): Settings => ({
  appearance: { theme: 'system', density: 'comfortable' },
  reader: { fontFamily: 'serif', fontSize: 18, measure: 66, lineHeight: 1.65 },
  refresh: { intervalMinutes: 30 },
  imageCacheCapBytes: 1024 ** 3,
  pruneReadDays: 90,
  pruneUnreadDays: 365,
});

const feed = { id: 5, title: 'Rust Blog', siteUrl: null, unread: 2, lastError: null, lastSuccessAt: null, favicon: null };
const sidebar: Sidebar = {
  allUnread: 2,
  starred: 0,
  today: 0,
  folders: [{ id: 2, name: 'Tech', unread: 2, feeds: [feed] }],
  feeds: [],
};

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 0),
  list: vi.fn(async (_q: ListQuery, offset: number, limit: number): Promise<ArticleRow[]> =>
    Array.from({ length: Math.max(0, Math.min(limit, 2 - offset)) }, (_, i) => ({
      id: offset + i + 1,
      feedId: 5,
      feedTitle: 'Rust Blog',
      title: `Article ${offset + i + 1}`,
      excerpt: 'excerpt',
      articleDate: 1_700_000_000 - offset - i,
      isRead: false,
      isStarred: false,
      leadImage: null,
      favicon: null,
    })),
  ),
  refresh: vi.fn(async () => true),
  sidebar: vi.fn(async (): Promise<Sidebar> => sidebar),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false })),
  layouts: vi.fn(async () => ({ default: 'list', views: {} })),
  setLayout: vi.fn(async () => {}),
  lastView: vi.fn(async () => null as string | null),
  setLastView: vi.fn(async () => {}),
  article: vi.fn(),
  setRead: vi.fn(async () => {}),
  markAllRead: vi.fn(async () => [] as number[]),
  deleteFolder: vi.fn(async () => {}),
  openExternal: vi.fn(async () => {}),
  checkForUpdates: vi.fn(),
  settings: vi.fn(async () => defaultSettings()),
  setSettings: vi.fn(async () => {}),
  hasSetting: vi.fn(async () => false),
  importOpml: vi.fn(async () => ({ added: 3, skipped: 1, foldersCreated: 1 })),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: dialogOpen, save: vi.fn(async () => null) }));

// The Assistant bridge (#122): a controllable setup plus fake runs we resolve by hand.
import type { AssistantEvent, AssistantResult, AssistantSetup } from '../assistant';
interface FakeTask {
  action: unknown;
  resolve: (r: AssistantResult) => void;
  reject: (e: unknown) => void;
}
const assistantTasks: FakeTask[] = [];
let assistantSetup: AssistantSetup | null = null;
vi.mock('../assistant', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../assistant')>()),
  getAssistantSetup: vi.fn(async () => assistantSetup),
  runAssistant: vi.fn((action: unknown, _onEvent: (e: AssistantEvent) => void) => {
    let resolve!: FakeTask['resolve'];
    let reject!: FakeTask['reject'];
    const result = new Promise<AssistantResult>((res, rej) => ((resolve = res), (reject = rej)));
    result.catch(() => {});
    assistantTasks.push({ action, resolve, reject });
    return { runId: `r${assistantTasks.length}`, result, cancel: vi.fn(async () => {}) };
  }),
}));

const { app } = await import('../state.svelte');
const { assist } = await import('../assistant-state.svelte');
const { runAssistant } = await import('../assistant');
const { default: ArticleList } = await import('./ArticleList.svelte');

let mounted: ReturnType<typeof mount> | null = null;
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

async function renderList() {
  mounted = mount(ArticleList, { target: document.body });
  flushSync();
  await settle();
  flushSync();
  return document.getElementById('pane-list') as HTMLElement;
}

beforeEach(() => {
  vi.clearAllMocks();
  assistantTasks.length = 0;
  assistantSetup = null;
  assist.reset();
  mockApi.count.mockResolvedValue(0);
  app.phase = 'ready';
  app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false };
  app.sidebar = sidebar;
  app.selection = { kind: 'allUnread' };
  app.search = '';
  app.searchAssisted = null;
  app.searchScoped = true;
  app.sort = 'newestFirst';
  app.total = 0;
  app.hiddenWithoutImage = 0;
  app.hasNewArticles = false;
  app.refresh = null;
  app.layouts = { default: 'list', views: {} };
  app.settings = defaultSettings();
  app.addFeedOpen = false;
  app.toasts = [];
  app.listEpoch = 0;
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  assist.reset();
  document.body.innerHTML = '';
});

const header = () => document.querySelector('#pane-list .header')!;
const h1 = () => header().querySelector('h1')!.textContent;
const buttonByLabel = (label: string) =>
  [...document.querySelectorAll<HTMLButtonElement>('#pane-list button')].find(
    (b) => b.getAttribute('aria-label') === label || b.title === label,
  );

describe('ArticleList header', () => {
  it('names the View and shows the Article count', async () => {
    app.total = 329;
    await renderList();
    expect(h1()).toBe('All Unread');
    expect(header().querySelector('.total')?.textContent).toBe('329');
  });

  it('names a Feed View after the Feed, and a search after the query', async () => {
    app.selection = { kind: 'feed', id: 5 };
    await renderList();
    expect(h1()).toBe('Rust Blog');
    app.search = '  rust  ';
    flushSync();
    expect(h1()).toBe('Search: rust');
  });

  it('hides the count when the Library setting says so (calm mode)', async () => {
    app.total = 7;
    app.settings = { ...defaultSettings(), appearance: { theme: 'system', density: 'comfortable', unreadCounts: 'hide' } };
    await renderList();
    expect(header().querySelector('.total')).toBeNull();
  });
});

async function openViewMenu() {
  const trigger = document.querySelector<HTMLButtonElement>('#pane-list button[aria-label^="View options"]')!;
  trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
  flushSync();
  await settle();
}
const radios = () => [...document.querySelectorAll<HTMLElement>('[role="menuitemradio"]')];
const radio = (name: string) => radios().find((r) => r.textContent?.trim() === name)!;
const choose = async (name: string) => {
  radio(name).dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
  flushSync();
  await settle();
};

describe('View menu', () => {
  it('replaces the Layout segments and sort toggle with one named button', async () => {
    app.layouts = { default: 'magazine', views: {} };
    await renderList();
    expect(document.querySelector('#pane-list .switcher')).toBeNull();
    const trigger = buttonByLabel('View options: Magazine, newest first')!;
    expect(trigger.title).toBe('View options: Magazine, newest first');
  });

  it('lists Layouts and sort orders as radio items reflecting the current state', async () => {
    app.layouts = { default: 'magazine', views: {} };
    await renderList();
    await openViewMenu();
    expect(radios().map((r) => r.textContent?.trim())).toEqual([
      'List', 'Cards', 'Magazine', 'Gallery', 'Newest first', 'Oldest first',
    ]);
    expect(radios().map((r) => r.getAttribute('aria-checked'))).toEqual([
      'false', 'false', 'true', 'false', 'true', 'false',
    ]);
    expect(document.querySelectorAll('[role="menu"] [role="group"]')).toHaveLength(2);
    // 1.4.1: the chosen items show a checkmark, not only bold text.
    expect(radios().map((r) => r.querySelector('.radio-mark svg') !== null)).toEqual([
      false, false, true, false, true, false,
    ]);
  });

  // Clara: each View remembers its own Layout, stored under the View key.
  it('stores the choice per View in the Library', async () => {
    await renderList();
    await openViewMenu();
    await choose('Cards');
    expect(mockApi.setLayout).toHaveBeenCalledWith('allUnread', 'cards');
    expect(app.layouts.views).toEqual({ allUnread: 'cards' });
    expect(app.layout).toBe('cards');
    expect(buttonByLabel('View options: Cards, newest first')).toBeDefined();

    app.selection = { kind: 'feed', id: 5 };
    flushSync();
    expect(app.layout, 'the Feed still uses the default').toBe('list');
    await openViewMenu();
    await choose('Gallery');
    expect(mockApi.setLayout).toHaveBeenCalledWith('feed:5', 'gallery');
    // Switching into Gallery re-filters the list (only Articles with a Lead Image).
    expect(mockApi.count).toHaveBeenCalled();
  });

  it('chooses the sort order and re-queries', async () => {
    await renderList();
    await openViewMenu();
    await choose('Oldest first');
    expect(app.sort).toBe('oldestFirst');
    expect(buttonByLabel('View options: List, oldest first')).toBeDefined();
    expect(mockApi.count).toHaveBeenCalledWith(expect.objectContaining({ sort: 'oldestFirst' }));
  });
});

describe('Search controls', () => {
  it('debounces search input and queries the search View', async () => {
    await renderList();
    const input = document.getElementById('search') as HTMLInputElement;
    input.value = 'rust';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(app.search, 'not yet — the debounce timer is running').toBe('');
    await vi.waitFor(() => expect(app.search).toBe('rust'), { timeout: 1000 });
    await settle();
    expect(mockApi.count).toHaveBeenCalledWith(expect.objectContaining({ search: 'rust' }));
  });

  it('scopes the search to the selected Feed until "search everywhere" is used', async () => {
    app.selection = { kind: 'feed', id: 5 };
    app.search = 'rust';
    await renderList();

    const chip = buttonByLabel('Searching in Rust Blog. Search everywhere instead')!;
    expect(chip.textContent).toContain('in Rust Blog');
    expect(app.query().view).toEqual({ kind: 'search', feed: 5 });

    chip.click();
    await settle();
    expect(app.query().view).toEqual({ kind: 'search' });
  });

  it('Escape in the search field clears the search', async () => {
    app.search = 'rust';
    await renderList();
    const input = document.getElementById('search') as HTMLInputElement;
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    await settle();
    expect(app.search).toBe('');
  });
});

describe('Empty states', () => {
  it('an empty Library offers adding a website or importing a list first (Mira)', async () => {
    app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 0, portable: false };
    await renderList();
    const empty = document.querySelector('#pane-list .empty')!;
    expect(empty.textContent).toContain('Add your first Feed');
    const buttons = [...empty.querySelectorAll('button')].map((b) => b.textContent?.trim());
    expect(buttons).toContain('Add a website or Feed…');
    expect(buttons).toContain('Import a list of Feeds…');

    (empty.querySelectorAll('button')[0] as HTMLButtonElement).click();
    flushSync();
    expect(app.addFeedOpen).toBe(true);
  });

  it('importing an OPML file reports what was added and skipped', async () => {
    app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 0, portable: false };
    await renderList();
    dialogOpen.mockResolvedValueOnce('/tmp/list.opml');
    const importBtn = [...document.querySelectorAll<HTMLButtonElement>('#pane-list .empty button')].find(
      (b) => b.textContent?.includes('Import'),
    )!;
    importBtn.click();
    await settle();
    await settle();
    expect(mockApi.importOpml).toHaveBeenCalledWith('/tmp/list.opml');
    expect(app.toasts.at(-1)?.message).toBe('3 Feeds added, 1 already present');
  });

  it('All Unread with nothing unread says "All caught up"', async () => {
    await renderList();
    expect(document.querySelector('#pane-list .empty')?.textContent).toContain('All caught up');
  });

  it('while a refresh runs, the empty state says Feeds are being fetched', async () => {
    app.refresh = { done: 1, total: 3, newArticles: 0, failed: 0, finished: false };
    await renderList();
    expect(document.querySelector('#pane-list .empty')?.textContent).toContain('Fetching your Feeds…');
  });

  it('a search without matches offers searching everywhere when scoped', async () => {
    app.selection = { kind: 'feed', id: 5 };
    app.search = 'xyz';
    await renderList();
    const empty = document.querySelector('#pane-list .empty')!;
    expect(empty.textContent).toContain('No matches in Rust Blog');
    const everywhere = [...empty.querySelectorAll('button')].find((b) => b.textContent?.includes('Search everywhere'))!;
    everywhere.click();
    await settle();
    expect(app.searchScope).toBeNull();
  });

  it('Gallery explains Articles hidden for having no Lead Image', async () => {
    app.layouts = { default: 'gallery', views: {} };
    // Gallery asks for Articles with a Lead Image only; the unfiltered count says how many are hidden.
    mockApi.count.mockImplementation(async (q: ListQuery) => (q.withLeadImage ? 2 : 5));
    await renderList();
    await app.resetList();
    await settle();
    flushSync();
    expect(app.total).toBe(2);
    expect(app.hiddenWithoutImage).toBe(3);
    expect(document.querySelector('#pane-list .note')?.textContent).toContain('3 Articles without images not shown');
  });
});

describe('Refresh progress and new Articles', () => {
  it('shows a progressbar while refreshing', async () => {
    app.refresh = { done: 5, total: 10, newArticles: 0, failed: 0, finished: false };
    await renderList();
    const bar = document.querySelector('[role="progressbar"]')!;
    expect(bar.getAttribute('aria-valuenow')).toBe('5');
    expect(bar.getAttribute('aria-valuemax')).toBe('10');
  });

  it('offers "New Articles · show" while reading and reloads on click', async () => {
    app.total = 2;
    mockApi.count.mockResolvedValue(2);
    await renderList();
    await app.resetList();
    await settle();
    app.hasNewArticles = true;
    flushSync();
    const pill = document.querySelector('.new-pill') as HTMLButtonElement;
    expect(pill.textContent).toContain('New Articles · show');
    mockApi.count.mockClear();
    pill.click();
    await settle();
    expect(mockApi.count, 'the list reloads').toHaveBeenCalled();
    expect(app.hasNewArticles).toBe(false);
  });
});

describe('Assistant search trigger (#122)', () => {
  const setup: AssistantSetup = { kind: 'claudeCode', path: '/bin/claude', model: null, disclosureAccepted: true };
  const trigger = () => buttonByLabel('Ask the Assistant to search');

  it('is hidden without a setup, and plain typing never calls an Assistant', async () => {
    await renderList();
    expect(trigger()).toBeUndefined();
    const input = document.getElementById('search') as HTMLInputElement;
    input.value = 'rust';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.waitFor(() => expect(app.search).toBe('rust'), { timeout: 1000 });
    await settle();
    expect(runAssistant).not.toHaveBeenCalled();
    expect(mockApi.count).toHaveBeenCalledWith(expect.objectContaining({ search: 'rust' }));
    const view = mockApi.count.mock.calls.at(-1)![0].view;
    expect(view, 'normal search stays plain text, no fts flag').toEqual({ kind: 'search' });
  });

  it('is hidden while the disclosure is not accepted', async () => {
    assistantSetup = { ...setup, disclosureAccepted: false };
    await renderList();
    await settle();
    expect(trigger()).toBeUndefined();
  });

  it('is shown with a setup; clicking sends the field text as the question', async () => {
    assistantSetup = setup;
    await renderList();
    await settle();
    flushSync();
    expect(trigger()).not.toBeNull();
    const input = document.getElementById('search') as HTMLInputElement;
    input.value = 'that post about WAL mode from last spring';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.waitFor(() => expect(app.search).toBe('that post about WAL mode from last spring'), { timeout: 1000 });
    trigger()!.click();
    await settle();
    expect(runAssistant).toHaveBeenCalledTimes(1);
    expect(assistantTasks[0]!.action).toEqual({ kind: 'searchQuery', question: 'that post about WAL mode from last spring' });
    expect(assist.searchRun).toBe('running');
    expect(trigger()!.hasAttribute('disabled')).toBe(true);
  });

  it('puts the generated query into the editable field and runs it as a raw FTS search', async () => {
    assistantSetup = setup;
    app.search = 'wal mode last spring';
    await renderList();
    await settle();
    trigger()!.click();
    await settle();
    assistantTasks[0]!.resolve({
      text: '{"query":"title:\\"wal\\"* AND sqlite","feed":5,"since":1740787200,"until":1748649599,"dropped":[]}',
      structured: { query: 'title:"wal"* AND sqlite', feed: 5, folder: null, since: 1_740_787_200, until: 1_748_649_599, dropped: [] },
    });
    await settle();
    flushSync();
    // Visible and editable: the field shows the generated query.
    const input = document.getElementById('search') as HTMLInputElement;
    expect(input.value).toBe('title:"wal"* AND sqlite');
    expect(app.search).toBe('title:"wal"* AND sqlite');
    // The search runs locally with the Assistant's scope and date range as a raw FTS query.
    expect(app.query().view).toEqual({ kind: 'search', feed: 5, since: 1_740_787_200, until: 1_748_649_599, fts: true });
    expect(mockApi.count).toHaveBeenCalledWith(expect.objectContaining({ search: 'title:"wal"* AND sqlite' }));
    // Honest label: the toast names the Assistant and says it may be wrong.
    expect(app.toasts.at(-1)?.message).toBe('Claude Code wrote this search — it may be wrong');
    expect(assist.searchRun).toBe('idle');
    // The scope chip names the Assistant-picked Feed.
    expect(buttonByLabel('Searching in Rust Blog. Search everywhere instead')).not.toBeNull();
    // Editing the generated query keeps it a raw FTS search.
    input.value = 'title:"wal"* AND sqlite OR wal2';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.waitFor(() => expect(app.search).toBe('title:"wal"* AND sqlite OR wal2'), { timeout: 1000 });
    await settle();
    expect(app.query().view).toEqual({ kind: 'search', feed: 5, since: 1_740_787_200, until: 1_748_649_599, fts: true });
  });

  it('does nothing on an empty field, and reports Assistant errors as a toast', async () => {
    assistantSetup = setup;
    await renderList();
    await settle();
    trigger()!.click();
    await settle();
    expect(runAssistant).not.toHaveBeenCalled();

    app.search = 'wal';
    flushSync();
    trigger()!.click();
    await settle();
    assistantTasks[0]!.reject({ kind: 'notSignedIn', message: 'raw' });
    await settle();
    flushSync();
    expect(assist.searchRun).toBe('error');
    expect(assist.searchError).toBe('notSignedIn');
    expect(app.toasts.at(-1)?.message).toContain("isn't signed in");
    expect(app.search, 'the field is untouched on error').toBe('wal');
  });
});
