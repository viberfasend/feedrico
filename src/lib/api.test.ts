// The typed bridge to the Rust shell: every command name and argument shape is pinned here,
// because the Rust core expects exactly these (Tauri camelCases nothing on its own — `new`
// below is literally the argument name the core reads). Components never call invoke() directly.
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ListQuery, Settings } from './api';

const invoke = vi.fn(async (_cmd: string, _args?: unknown) => undefined);
const convertFileSrc = vi.fn((key: string, protocol: string) => `${protocol}://localhost/${key}`);
const listen = vi.fn(async (_event: string, _cb: (e: { payload: unknown }) => void) => () => {});

vi.mock('@tauri-apps/api/core', () => ({ invoke, convertFileSrc }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const { api, isCommandError, mediaUrl, todaySince } = await import('./api');

const query: ListQuery = { view: { kind: 'allUnread' }, sort: 'newestFirst' };
const settings = { appearance: { theme: 'dark', density: 'compact' } } as unknown as Settings;

beforeEach(() => {
  invoke.mockClear();
  listen.mockClear();
});

describe('isCommandError', () => {
  it('accepts the { kind, message } shape the core rejects with', () => {
    expect(isCommandError({ kind: 'locked', message: 'in use' })).toBe(true);
  });
  it('rejects everything else', () => {
    for (const e of [null, undefined, 'locked', 42, new Error('x'), { kind: 'locked' }, { message: 'x' }]) {
      expect(isCommandError(e)).toBe(false);
    }
  });
});

describe('mediaUrl', () => {
  // The reader serves images only from the local cache via the shell's media protocol (Paul:
  // opening an Article must cause no network request).
  it('builds the URL through the media protocol', () => {
    expect(mediaUrl('abc123')).toBe('media://localhost/abc123');
    expect(convertFileSrc).toHaveBeenCalledWith('abc123', 'media');
  });
});

describe('todaySince', () => {
  // The Today Smart View starts at local midnight (GLOSSARY: Article Date since local midnight).
  it('is local midnight of the given day in unix seconds', () => {
    const now = new Date(2026, 10, 7, 15, 42, 11);
    const midnight = new Date(2026, 10, 7, 0, 0, 0, 0);
    expect(todaySince(now)).toBe(Math.floor(midnight.getTime() / 1000));
  });

  it('stays the same all day and moves at midnight', () => {
    const morning = new Date(2026, 10, 7, 0, 0, 0, 0);
    const evening = new Date(2026, 10, 7, 23, 59, 59);
    const nextDay = new Date(2026, 10, 8, 0, 0, 0, 0);
    expect(todaySince(morning)).toBe(todaySince(evening));
    expect(todaySince(nextDay)).toBe(todaySince(morning) + 86_400);
  });
});

describe('api command bridge', () => {
  it.each([
    ['startup', [], 'startup', undefined],
    ['recentLibraries', [], 'recent_libraries', undefined],
    ['inspect', ['/lib'], 'library_inspect', { path: '/lib' }],
    ['create', ['/lib'], 'library_create', { path: '/lib' }],
    ['open', ['/lib'], 'library_open', { path: '/lib', allowUpgrade: false, takeOverLock: false }],
    ['open', ['/lib', true, true], 'library_open', { path: '/lib', allowUpgrade: true, takeOverLock: true }],
    ['info', [], 'library_info', undefined],
    ['close', [], 'library_close', undefined],
    ['importOpml', ['/tmp/a.opml'], 'import_opml', { path: '/tmp/a.opml' }],
    ['discoverFeed', ['example.com'], 'discover_feed', { input: 'example.com' }],
    ['addFeed', ['https://f', 'F', 'https://s', 3], 'add_feed', { url: 'https://f', title: 'F', siteUrl: 'https://s', folderId: 3 }],
    ['renameFeed', [1, 'New'], 'rename_feed', { id: 1, title: 'New' }],
    ['renameFolder', [2, 'Tech'], 'rename_folder', { id: 2, name: 'Tech' }],
    ['moveFeed', [1, null], 'move_feed', { id: 1, folderId: null }],
    ['deleteFolder', [2, true], 'delete_folder', { id: 2, unsubscribeFeeds: true }],
    ['unsubscribe', [5], 'unsubscribe', { id: 5 }],
    ['unsubscribeFeeds', [[5, 6]], 'unsubscribe_feeds', { ids: [5, 6] }],
    ['retryFeed', [5], 'retry_feed', { id: 5 }],
    ['exportOpml', ['/tmp/a.opml'], 'export_opml', { path: '/tmp/a.opml' }],
    ['exportLibrary', ['/bak'], 'export_library', { path: '/bak' }],
    ['sidebar', [1_700_000_000], 'sidebar', { todaySince: 1_700_000_000 }],
    ['count', [query], 'count_articles', { query }],
    ['list', [query, 0, 100], 'list_articles', { query, offset: 0, limit: 100 }],
    ['article', [7], 'get_article', { id: 7 }],
    ['setRead', [7, true], 'set_read', { id: 7, read: true }],
    ['setStarred', [7, false], 'set_starred', { id: 7, starred: false }],
    ['markAllRead', [query], 'mark_all_read', { query }],
    ['markUnread', [[1, 2]], 'mark_unread', { ids: [1, 2] }],
    ['layouts', [], 'layouts', undefined],
    ['setLayout', ['feed:3', 'cards'], 'set_layout', { view: 'feed:3', layout: 'cards' }],
    ['lastView', [], 'last_view', undefined],
    ['setLastView', ['feed:3'], 'set_last_view', { view: 'feed:3' }],
    ['settings', [], 'settings', undefined],
    ['setSettings', [settings], 'set_settings', { new: settings }],
    ['hasSetting', ['appearance.theme'], 'has_setting', { key: 'appearance.theme' }],
    ['librarySize', [], 'library_size', undefined],
    ['mediaSize', [], 'media_size', undefined],
    ['autostartEnabled', [], 'autostart_enabled', undefined],
    ['setAutostart', [true], 'set_autostart', { enabled: true }],
    ['openExternal', ['https://x'], 'open_external', { url: 'https://x' }],
    ['checkForUpdates', [], 'check_for_updates', undefined],
    ['refresh', [true], 'refresh', { force: true }],
    ['perfInteractive', [], 'perf_interactive', undefined],
    ['perfFinish', [{ n: 1 }], 'perf_finish', { report: { n: 1 } }],
  ] as const)('%s(...%j) invokes %s with the exact arguments', async (method, args, command, expected) => {
    await (api[method] as (...a: unknown[]) => unknown)(...args);
    // Commands without arguments are invoked with the command name alone.
    expect(invoke.mock.calls.at(-1)).toEqual(expected === undefined ? [command] : [command, expected]);
  });

  it('wraps event listeners and unwraps their payload', async () => {
    let seen: unknown;
    const unlisten = await api.onRefreshProgress((p) => (seen = p));
    const call = listen.mock.calls.find(([event]) => event === 'refresh-progress')?.[1]!;
    call({ payload: { done: 1, total: 2, newArticles: 0, failed: 0, finished: false } });
    expect(seen).toEqual({ done: 1, total: 2, newArticles: 0, failed: 0, finished: false });
    expect(typeof unlisten).toBe('function');

    await api.onMediaCached(() => (seen = 'cached'));
    listen.mock.calls.find(([event]) => event === 'media-cached')?.[1]!({ payload: null });
    expect(seen).toBe('cached');

    await api.onArticlesPruned(() => (seen = 'pruned'));
    listen.mock.calls.find(([event]) => event === 'articles-pruned')?.[1]!({ payload: null });
    expect(seen).toBe('pruned');

    await api.onExportProgress((p) => (seen = p));
    listen.mock.calls.find(([event]) => event === 'export-progress')?.[1]!({ payload: { done: 3, total: 9 } });
    expect(seen).toEqual({ done: 3, total: 9 });
  });
});
