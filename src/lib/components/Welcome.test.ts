// @vitest-environment happy-dom
// The welcome screen is Mira's (The Newcomer) first step: create a Library in the default place
// or anywhere else, open an existing one, or jump back via Recent Libraries. No jargon, and
// every button is disabled while a task runs so nothing is started twice.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ListQuery, Sidebar } from '../api';

const dialogOpen = vi.fn(async (_opts: unknown) => null as string | null);
const join = vi.fn(async (...parts: string[]) => parts.join('/'));

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '/data/Feedrico Library' })),
  recentLibraries: vi.fn(async () => ['/stick/Feedrico Library', '/data/Feedrico Library'] as string[]),
  create: vi.fn(async (_path: string) => ({ path: '/data/Feedrico Library', name: 'Lib', formatVersion: 3, feedCount: 0, portable: false })),
  open: vi.fn(async (_path: string) => ({ path: '/stick/Feedrico Library', name: 'Lib', formatVersion: 3, feedCount: 2, portable: true })),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 0, portable: false })),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 0),
  list: vi.fn(async (_q: ListQuery, _o: number, _l: number) => []),
  refresh: vi.fn(async () => false),
  sidebar: vi.fn(async (): Promise<Sidebar> => ({ allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] })),
  layouts: vi.fn(async () => ({ default: 'list', views: {} })),
  lastView: vi.fn(async () => null as string | null),
  setLastView: vi.fn(async () => {}),
  setLayout: vi.fn(async () => {}),
  article: vi.fn(),
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
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: dialogOpen, save: vi.fn() }));
vi.mock('@tauri-apps/api/path', () => ({ join }));

const { app } = await import('../state.svelte');
const { default: Welcome } = await import('./Welcome.svelte');

let mounted: ReturnType<typeof mount> | null = null;
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  vi.clearAllMocks();
  app.phase = 'welcome';
  app.defaultPath = '/data/Feedrico Library';
  app.recent = ['/stick/Feedrico Library', '/data/Feedrico Library'];
  app.toasts = [];
  mounted = mount(Welcome, { target: document.body });
  flushSync();
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

const button = (label: string) =>
  [...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.includes(label))!;

describe('Welcome screen (first run)', () => {
  it('offers one obvious primary step: create a new Library in the default place', async () => {
    button('Create new Library').click();
    await settle();
    expect(mockApi.create).toHaveBeenCalledWith('/data/Feedrico Library');
    expect(app.phase).toBe('ready');
    expect(app.library?.path).toBe('/data/Feedrico Library');
  });

  it('explains the default location in plain words, keeping the path in the title', () => {
    expect(document.body.textContent).toContain('app data folder');
    expect(document.querySelector('.where')?.getAttribute('title')).toBe('/data/Feedrico Library');
  });

  it('creates the Library in a chosen folder when "another place" is picked', async () => {
    dialogOpen.mockResolvedValueOnce('/media/stick');
    button('Create in another place…').click();
    await settle();
    await settle();
    expect(dialogOpen).toHaveBeenCalledWith({ directory: true, title: 'Where should your Library live?' });
    expect(join).toHaveBeenCalledWith('/media/stick', 'Feedrico Library');
    expect(mockApi.create).toHaveBeenCalledWith('/media/stick/Feedrico Library');
  });

  it('opens an existing Library from a picked folder', async () => {
    dialogOpen.mockResolvedValueOnce('/stick/Feedrico Library');
    button('Open existing Library…').click();
    await settle();
    await settle();
    expect(dialogOpen).toHaveBeenCalledWith({ directory: true, title: 'Open a Feedrico Library' });
    expect(mockApi.open).toHaveBeenCalledWith('/stick/Feedrico Library', false, false);
    expect(app.phase).toBe('ready');
  });

  it('lists Recent Libraries and opens one on click', async () => {
    const recent = [...document.querySelectorAll('.recent .link')].map((b) => b.textContent);
    expect(recent).toEqual(['/stick/Feedrico Library', '/data/Feedrico Library']);
    (document.querySelectorAll('.recent .link')[0] as HTMLButtonElement).click();
    await settle();
    expect(mockApi.open).toHaveBeenCalledWith('/stick/Feedrico Library', false, false);
  });

  it('cancelling the folder picker changes nothing', async () => {
    dialogOpen.mockResolvedValueOnce(null);
    button('Open existing Library…').click();
    await settle();
    expect(mockApi.open).not.toHaveBeenCalled();
    expect(app.phase).toBe('welcome');
  });

  it('disables every button while a task is running', async () => {
    let resolveCreate!: (v: Awaited<ReturnType<typeof mockApi.create>>) => void;
    mockApi.create.mockReturnValueOnce(new Promise((res) => (resolveCreate = res)));
    button('Create new Library').click();
    flushSync();
    for (const b of document.querySelectorAll('button')) {
      expect(b.disabled, `"${b.textContent?.trim()}" is disabled while busy`).toBe(true);
    }
    resolveCreate({ path: '/data/Feedrico Library', name: 'Lib', formatVersion: 3, feedCount: 0, portable: false });
    await settle();
  });

  it('hides the primary button and Recent Libraries when there is nowhere to default to', () => {
    app.defaultPath = '';
    app.recent = [];
    flushSync();
    expect(button('Create new Library')).toBeUndefined();
    expect(document.querySelector('.recent')).toBeNull();
    expect(button('Create in another place…')).toBeDefined();
    expect(button('Open existing Library…')).toBeDefined();
  });
});
