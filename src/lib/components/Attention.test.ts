// @vitest-environment happy-dom
// The attention screen decides what happens when a Library can't just be opened: stale lock,
// older format, newer format, not a Library. Version scenario matrix (docs/personas.md):
// Jonas (The Carrier) pulls the stick mid-session (D), Ute (The Host) opens a newer Library
// with an older app (B — nothing may be written), and an older Library gets the upgrade offer (A).
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ListQuery, Sidebar } from '../api';

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  inspect: vi.fn(),
  open: vi.fn(),
  create: vi.fn(),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 0),
  list: vi.fn(async (_q: ListQuery, _o: number, _l: number) => []),
  refresh: vi.fn(async () => true),
  sidebar: vi.fn(async (): Promise<Sidebar> => ({ allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] })),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 0, portable: false })),
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

const { app } = await import('../state.svelte');
const { default: Attention } = await import('./Attention.svelte');

let mounted: ReturnType<typeof mount> | null = null;

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  vi.clearAllMocks();
  app.phase = 'attention';
  app.attention = null;
  mounted = mount(Attention, { target: document.body });
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  app.attention = null;
  app.phase = 'welcome';
  document.body.innerHTML = '';
});

function show(path: string, status: Parameters<typeof app.needsAttention>[1], takeOverLock = false) {
  app.needsAttention(path, status, takeOverLock);
  flushSync();
  return document.querySelector('[role="alertdialog"]') as HTMLElement;
}

/** textContent with the template's line breaks collapsed, for phrase matching. */
const words = (el: Element | null) => el?.textContent?.replace(/\s+/g, ' ') ?? '';

const button = (label: string) =>
  [...document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')].find(
    (b) => b.textContent?.trim() === label,
  )!;

describe('Attention screen', () => {
  it('is an alertdialog labelled by its title', () => {
    const card = show('/stick/Feedrico Library', { kind: 'locked', machine: 'home-pc', pid: 1, openedAt: 0 });
    expect(card.getAttribute('aria-labelledby')).toBe('attention-title');
    expect(document.getElementById('attention-title')).not.toBeNull();
  });

  // Scenario D: the stick was pulled mid-session; back home the lock is stale.
  it('offers "Open anyway" for a stale lock and takes the lock over', async () => {
    const openedAt = 1_763_000_000;
    show('/stick/Feedrico Library', { kind: 'locked', machine: 'friend-laptop', pid: 9, openedAt });
    const card = document.querySelector('[role="alertdialog"]')!;
    expect(words(card)).toContain('friend-laptop');
    expect(words(card)).toContain(new Date(openedAt * 1000).toLocaleString());
    expect(words(card)).toContain("wasn't closed cleanly");

    mockApi.open.mockResolvedValue({ path: '/stick/Feedrico Library', name: 'Lib', formatVersion: 3, feedCount: 0, portable: true });
    button('Open anyway').click();
    await settle();
    expect(mockApi.open).toHaveBeenCalledWith('/stick/Feedrico Library', false, true);
  });

  it('lets the user choose another Library instead', async () => {
    show('/lib', { kind: 'locked', machine: 'x', pid: 1, openedAt: 0 });
    button('Choose another Library').click();
    await settle();
    expect(app.phase).toBe('welcome');
    expect(mockApi.recentLibraries).toHaveBeenCalled();
    expect(mockApi.open).not.toHaveBeenCalled();
  });

  // Scenario A: newer app, older Library — ask before upgrading, remember a lock takeover.
  it('asks about the upgrade and keeps an earlier "Open anyway"', async () => {
    show('/lib', { kind: 'needsUpgrade', found: 2, target: 3 }, true);
    const card = document.querySelector('[role="alertdialog"]')!;
    expect(card.querySelector('h1')?.textContent).toBe('Upgrade this Library?');
    expect(words(card)).toContain("older versions of Feedrico won't open it anymore");

    mockApi.open.mockResolvedValue({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 0, portable: false });
    button('Upgrade and open').click();
    await settle();
    expect(mockApi.open).toHaveBeenCalledWith('/lib', true, true);
  });

  // Scenario B: older app, newer Library — refuse, and never write anything.
  it('refuses a newer Library and offers no action that could write to it', () => {
    const card = show('/stick/Feedrico Library', { kind: 'tooNew', found: 4, supported: 3 });
    expect(card.querySelector('h1')?.textContent).toBe('This Library needs a newer Feedrico');
    expect(words(card)).toContain('format 4');
    expect(words(card)).toContain('supports up to 3');
    expect(words(card)).toContain('Nothing was changed');
    const labels = [...card.querySelectorAll('button')].map((b) => b.textContent?.trim());
    expect(labels).toEqual(['Choose another Library']);
    expect(mockApi.open).not.toHaveBeenCalled();
  });

  it('explains when a folder is not a Library at all', async () => {
    const card = show('/home/me/Documents', { kind: 'notALibrary' });
    expect(card.querySelector('h1')?.textContent).toBe("This folder isn't a Feedrico Library");
    expect(words(card)).toContain('/home/me/Documents');
    button('Back').click();
    await settle();
    expect(app.phase).toBe('welcome');
  });

  it('renders nothing when there is no attention state', () => {
    app.attention = null;
    flushSync();
    expect(document.querySelector('[role="alertdialog"]')).toBeNull();
  });
});
