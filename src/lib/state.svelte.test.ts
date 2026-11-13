// @vitest-environment happy-dom
// Regression tests for the app state. Each test names the bug it guards against.
import { flushSync } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ArticleRow, ListQuery, Sidebar } from './api';

/** A promise we resolve by hand, to replay exact orderings of IPC responses. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const counts: ReturnType<typeof deferred<number>>[] = [];
const lists: { query: ListQuery; offset: number; d: ReturnType<typeof deferred<ArticleRow[]>> }[] = [];

const mockApi = {
  count: vi.fn((_query: ListQuery) => {
    const d = deferred<number>();
    counts.push(d);
    return d.promise;
  }),
  list: vi.fn((query: ListQuery, offset: number, _limit: number) => {
    const d = deferred<ArticleRow[]>();
    lists.push({ query, offset, d });
    return d.promise;
  }),
  open: vi.fn(),
  inspect: vi.fn(),
  setLayout: vi.fn(async () => {}),
  article: vi.fn(),
  setRead: vi.fn(async () => {}),
  sidebar: vi.fn(async (): Promise<Sidebar> => ({ allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] })),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 1, feedCount: 1, portable: false })),
  deleteFolder: vi.fn(async (_id: number, _unsubscribeFeeds: boolean) => {}),
  layouts: vi.fn(async () => ({ default: 'list', views: {} })),
  settings: vi.fn(async () => ({
    appearance: { theme: 'system', density: 'comfortable' },
    reader: { fontFamily: 'serif', fontSize: 18, measure: 66, lineHeight: 1.65 },
    refresh: { intervalMinutes: 30 },
    imageCacheCapBytes: 1024 * 1024 * 1024,
    pruneReadDays: 90,
    pruneUnreadDays: 365,
  })),
  setSettings: vi.fn(async () => {}),
  hasSetting: vi.fn(async () => false),
  librarySize: vi.fn(async () => 0),
  mediaSize: vi.fn(async () => 0),
  autostartEnabled: vi.fn(async () => false),
  setAutostart: vi.fn(async () => {}),
  refresh: vi.fn(async () => true),
  onRefreshProgress: vi.fn(async () => () => {}),
  openExternal: vi.fn(async () => {}),
  appVersion: vi.fn(async () => '0.1.0'),
  checkForUpdates: vi.fn(),
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
};

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  api: mockApi,
}));

const { AppState, parseSelection, selectionKey } = await import('./state.svelte');

function rows(count: number, firstId = 1): ArticleRow[] {
  return Array.from({ length: count }, (_, i) => ({
    id: firstId + i,
    feedId: 1,
    feedTitle: 'Feed',
    title: `Article ${firstId + i}`,
    excerpt: '',
    articleDate: 1_700_000_000 - i,
    isRead: false,
    isStarred: false,
    leadImage: null,
    favicon: null,
  }));
}

/** Lets pending promise callbacks run. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  counts.length = 0;
  lists.length = 0;
  vi.clearAllMocks();
});

describe('Article list loading', () => {
  // Bug (Gallery showed only placeholders): a grid mounted while the list was being reset
  // requested page 0; the reset then waited on that request, discarded its result as stale,
  // and left its loading marker behind, so page 0 never loaded.
  it('loads rows requested while a reset is still counting', async () => {
    const app = new AppState();
    const reset = app.resetList();
    void app.ensureRange(0, 10); // the new Layout asks for rows before the count arrives
    counts[0]!.resolve(329);
    await settle();
    for (const { d } of lists) d.resolve(rows(100));
    await reset;
    await settle();

    expect(app.total).toBe(329);
    expect(app.rowAt(0)?.id).toBe(1);
  });

  it('ignores late answers from a previous list and never gets stuck on them', async () => {
    const app = new AppState();
    app.total = 50;
    void app.ensureRange(0, 10); // request for the *old* list, still in flight
    const stale = lists[0]!;

    const reset = app.resetList();
    counts[0]!.resolve(20);
    await settle();
    const fresh = lists.find((l) => l !== stale);
    expect(fresh, 'the new list must issue its own request').toBeDefined();
    fresh!.d.resolve(rows(20, 100));
    await reset;

    stale.d.resolve(rows(10, 1)); // arrives last; must not overwrite the new list
    await settle();
    expect(app.rowAt(0)?.id).toBe(100);
  });

  it('retries a page whose request failed', async () => {
    const app = new AppState();
    app.total = 10;
    const first = app.ensureRange(0, 10);
    lists[0]!.d.reject(new Error('IPC down'));
    await first.catch(() => {});

    void app.ensureRange(0, 10);
    expect(lists).toHaveLength(2);
    lists[1]!.d.resolve(rows(10));
    await settle();
    expect(app.rowAt(0)?.id).toBe(1);
  });

  // Bug (list showed only placeholders): rows never re-rendered after their page arrived,
  // because nothing subscribed to page loads. rowAt() itself must be reactive.
  it('re-runs effects that read rowAt() when the page arrives', async () => {
    const app = new AppState();
    app.total = 10;
    const seen: (number | undefined)[] = [];
    const stop = $effect.root(() => {
      $effect(() => {
        seen.push(app.rowAt(0)?.id);
      });
    });
    flushSync();
    void app.ensureRange(0, 10);
    lists[0]!.d.resolve(rows(10, 7));
    await settle();
    flushSync();
    stop();

    expect(seen).toEqual([undefined, 7]);
  });

  it('asks for Articles with a Lead Image only in Gallery', async () => {
    const app = new AppState();
    app.layouts = { default: 'gallery', views: {} };
    expect(app.query().withLeadImage).toBe(true);
    app.layouts = { default: 'cards', views: { allUnread: 'list' } };
    expect(app.query().withLeadImage).toBe(false);
    expect(app.layout).toBe('list');
  });
});

describe('Favicons', () => {
  // Favicons are cached after a refresh, usually after the list is already on screen. The
  // rows must pick them up without reloading the list (which would lose the scroll position).
  it('shows favicons cached after the list loaded, without reloading the list', async () => {
    const app = new AppState();
    app.total = 2;
    void app.ensureRange(0, 2);
    lists[0]!.d.resolve([...rows(1, 1), { ...rows(1, 2)[0]!, feedId: 2 }]);
    await settle();
    const feed = (id: number, favicon: string | null) => ({
      id,
      title: `Feed ${id}`,
      siteUrl: null,
      unread: 1,
      lastError: null,
      lastSuccessAt: null,
      favicon,
    });
    mockApi.sidebar.mockResolvedValueOnce({
      allUnread: 2,
      starred: 0,
      today: 0,
      folders: [],
      feeds: [feed(1, 'a'.repeat(32)), feed(2, null)],
    });

    await app.onMediaCached();

    expect(app.rowAt(0)?.favicon).toBe('a'.repeat(32));
    expect(app.rowAt(1)?.favicon).toBeNull();
    expect(lists, 'no list reload').toHaveLength(1);
  });
});

describe('Opening a Library', () => {
  // Bug (couldn't press "Open anyway"): taking over a stale lock failed on the pending
  // upgrade, re-inspecting reported the untouched lock again, and the screen looped.
  it('asks about the upgrade after the lock was taken over, and keeps the takeover', async () => {
    const app = new AppState();
    mockApi.open.mockRejectedValueOnce({ kind: 'needsUpgrade', message: 'needs upgrade' });
    mockApi.inspect.mockResolvedValue({ kind: 'locked', machine: 'here', pid: 1, openedAt: 0 });

    await app.open('/lib', false, true);

    expect(app.phase).toBe('attention');
    expect(app.attention?.status.kind).toBe('needsUpgrade');
    expect(app.attention?.takeOverLock).toBe(true);
    expect(mockApi.inspect).not.toHaveBeenCalled();
  });
});

describe('Deleting a Folder', () => {
  // Bug (YouTube Folder deleted, its Articles still in All Unread): deleting a Folder could
  // only move its Feeds to the top level. Unsubscribing them must also drop their Articles
  // from the list on screen and update the Library's Feed count.
  it('unsubscribes from its Feeds and reloads the list when asked to', async () => {
    const app = new AppState();
    const done = app.deleteFolder(7, true);
    await vi.waitFor(() => expect(counts).toHaveLength(1));
    counts[0]!.resolve(0);
    await vi.waitFor(() => expect(lists).toHaveLength(1));
    lists[0]!.d.resolve([]);
    await done;

    expect(mockApi.deleteFolder).toHaveBeenCalledWith(7, true);
    expect(mockApi.info).toHaveBeenCalled();
    expect(mockApi.count).toHaveBeenCalled();
  });

  it('keeps the Feeds and the list when they move to the top level', async () => {
    const app = new AppState();
    await app.deleteFolder(7, false);

    expect(mockApi.deleteFolder).toHaveBeenCalledWith(7, false);
    expect(mockApi.info).not.toHaveBeenCalled();
    expect(mockApi.count).not.toHaveBeenCalled();
  });
});

describe('Perf mode', () => {
  // The perf harness opens a generated Library whose Feeds point at example domains. A refresh
  // would hit the network, write errors into the fixture and skew every timing.
  it('never refreshes Feeds', async () => {
    vi.useFakeTimers();
    try {
      const app = new AppState();
      app.perf = true;
      mockApi.open.mockResolvedValueOnce({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 300, portable: false });
      const opened = app.open('/lib');
      await vi.waitFor(() => expect(counts).toHaveLength(1));
      counts[0]!.resolve(0);
      await vi.waitFor(() => expect(lists).toHaveLength(1));
      lists[0]!.d.resolve([]);
      await opened;
      await vi.advanceTimersByTimeAsync(2 * 60 * 60 * 1000);

      expect(mockApi.refresh).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it('lets the harness await a View switch until its first page is loaded', async () => {
    const app = new AppState();
    let done = false;
    const switched = app.select({ kind: 'starred' }).then(() => (done = true));
    await vi.waitFor(() => expect(counts).toHaveLength(1));
    counts[0]!.resolve(1);
    await vi.waitFor(() => expect(lists).toHaveLength(1));
    expect(done).toBe(false);
    lists[0]!.d.resolve(rows(1));
    await switched;
    expect(app.rowAt(0)?.id).toBe(1);
  });
});

describe('Check for updates', () => {
  it('says the app is up to date', async () => {
    const app = new AppState();
    mockApi.checkForUpdates.mockResolvedValue({ kind: 'upToDate', current: '0.1.0' });
    await app.checkForUpdates();
    expect(app.toasts.at(-1)?.message).toBe("You're up to date (0.1.0)");
    expect(app.toasts.at(-1)?.action).toBeUndefined();
  });

  it('offers the new release with an Open action that opens the release page', async () => {
    const app = new AppState();
    mockApi.checkForUpdates.mockResolvedValue({
      kind: 'available',
      current: '0.1.0',
      latest: '0.2.0',
      url: 'https://github.com/viberfasend/feedrico/releases/tag/v0.2.0',
    });
    await app.checkForUpdates();
    const toast = app.toasts.at(-1);
    expect(toast?.message).toBe('Feedrico 0.2.0 is available');
    expect(toast?.action?.label).toBe('Open');
    toast?.action?.run();
    expect(mockApi.openExternal).toHaveBeenCalledWith(
      'https://github.com/viberfasend/feedrico/releases/tag/v0.2.0',
    );
  });

  it('reads a 404 as "no release published yet"', async () => {
    const app = new AppState();
    mockApi.checkForUpdates.mockResolvedValue({ kind: 'noReleases', current: '0.1.0' });
    await app.checkForUpdates();
    expect(app.toasts.at(-1)?.message).toBe('No release published yet');
  });

  it('shows a clear message when GitHub cannot be reached', async () => {
    const app = new AppState();
    mockApi.checkForUpdates.mockRejectedValue({ kind: 'internal', message: 'network error: could not connect' });
    await app.checkForUpdates();
    expect(app.toasts.at(-1)?.message).toBe(
      "Couldn't reach GitHub. Check your connection and try again.",
    );
  });

  it('ignores a second click while a check is still in flight', async () => {
    const app = new AppState();
    const d = deferred<unknown>();
    mockApi.checkForUpdates.mockReturnValue(d.promise);
    const first = app.checkForUpdates();
    void app.checkForUpdates();
    void app.checkForUpdates();
    expect(mockApi.checkForUpdates).toHaveBeenCalledTimes(1);
    d.resolve({ kind: 'upToDate', current: '0.1.0' });
    await first;
    expect(app.toasts.at(-1)?.message).toBe("You're up to date (0.1.0)");
    // Once finished, clicking again works again.
    mockApi.checkForUpdates.mockResolvedValue({ kind: 'noReleases', current: '0.1.0' });
    await app.checkForUpdates();
    expect(mockApi.checkForUpdates).toHaveBeenCalledTimes(2);
  });

  // Network policy: nothing may call the update check unless the user clicks.
  it('never checks for updates on startup', async () => {
    const app = new AppState();
    await app.start();
    expect(mockApi.checkForUpdates).not.toHaveBeenCalled();
  });

  // Spec "Search": scoped to everything, a Folder or a Feed (#63). It used to ignore the
  // selected Folder or Feed and always search the whole Library.
  it('searches inside the selected Folder or Feed until the scope is removed', () => {
    const app = new AppState();
    app.selection = { kind: 'folder', id: 3 };
    app.search = 'rust';
    expect(app.query().view).toEqual({ kind: 'search', folder: 3 });
    app.selection = { kind: 'feed', id: 42 };
    expect(app.query().view).toEqual({ kind: 'search', feed: 42 });
    void app.searchEverywhere();
    expect(app.query().view).toEqual({ kind: 'search' });
    void app.select({ kind: 'feed', id: 42 });
    app.search = 'rust';
    expect(app.query().view, 'a new selection scopes again').toEqual({ kind: 'search', feed: 42 });
    app.selection = { kind: 'allUnread' };
    expect(app.query().view, 'Smart Views search everything').toEqual({ kind: 'search' });
  });

  // #62: every start opened All Unread. The last View is stored in the Library as a View key.
  it('turns Selections into View keys and back, rejecting unknown keys', () => {
    for (const s of [{ kind: 'allUnread' }, { kind: 'today' }, { kind: 'starred' }, { kind: 'feed', id: 7 }, { kind: 'folder', id: 2 }] as const) {
      expect(parseSelection(selectionKey(s))).toEqual(s);
    }
    expect(selectionKey({ kind: 'feed', id: 7 })).toBe('feed:7');
    for (const bad of [null, '', 'search', 'feed:', 'feed:x', 'tag:1']) expect(parseSelection(bad)).toBeNull();
  });
});
