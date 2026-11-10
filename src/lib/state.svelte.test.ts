// @vitest-environment happy-dom
// Regression tests for the app state. Each test names the bug it guards against.
import { flushSync } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ArticleRow, ListQuery } from './api';

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
  sidebar: vi.fn(async () => ({ allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] })),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 1, feedCount: 1, portable: false })),
  deleteFolder: vi.fn(async (_id: number, _unsubscribeFeeds: boolean) => {}),
};

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  api: mockApi,
}));

const { AppState } = await import('./state.svelte');

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
