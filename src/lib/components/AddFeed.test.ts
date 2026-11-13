// @vitest-environment happy-dom
// Add Feed dialog (Mira, The Newcomer): paste a normal website address, not a feed URL —
// autodiscovery finds the Feed. Several candidates are offered as a choice. A failure says what
// went wrong (role="alert") and keeps the dialog open. Adding lands in the chosen Folder.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AddOutcome, ListQuery, Sidebar } from '../api';

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 0),
  list: vi.fn(async (_q: ListQuery, _o: number, _l: number) => []),
  refresh: vi.fn(async () => true),
  sidebar: vi.fn(async (): Promise<Sidebar> => ({ allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] })),
  info: vi.fn(async () => ({ path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false })),
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
  discoverFeed: vi.fn(),
  addFeed: vi.fn(async (): Promise<AddOutcome> => ({ kind: 'added', feedId: 9 })),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

const { app } = await import('../state.svelte');
const { default: AddFeed } = await import('./AddFeed.svelte');

let mounted: ReturnType<typeof mount> | null = null;
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  vi.clearAllMocks();
  mockApi.addFeed.mockResolvedValue({ kind: 'added', feedId: 9 });
  app.phase = 'ready';
  app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 0, portable: false };
  app.sidebar = {
    allUnread: 0,
    starred: 0,
    today: 0,
    folders: [{ id: 3, name: 'Tech', unread: 0, feeds: [] }],
    feeds: [],
  };
  app.toasts = [];
  app.addFeedOpen = true;
  mounted = mount(AddFeed, { target: document.body });
  flushSync();
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  app.addFeedOpen = false;
  document.body.innerHTML = '';
});

const urlInput = () => document.getElementById('add-feed-url') as HTMLInputElement;
const submitButton = () =>
  [...document.querySelectorAll<HTMLButtonElement>('button[type="submit"]')][0]!;

async function typeAddress(value: string) {
  urlInput().value = value;
  urlInput().dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  await settle();
}

async function submit() {
  submitButton().click();
  flushSync();
  await settle();
  await settle();
}

describe('Add Feed dialog', () => {
  it('keeps Add disabled until an address is typed', async () => {
    expect(submitButton().disabled).toBe(true);
    await typeAddress('example.com');
    expect(submitButton().disabled).toBe(false);
  });

  it('discovers the Feed behind a plain website address and subscribes to it', async () => {
    mockApi.discoverFeed.mockResolvedValue({
      kind: 'found',
      url: 'https://example.com/feed.xml',
      title: 'Example Blog',
      siteUrl: 'https://example.com',
    });
    await typeAddress('example.com');
    await submit();

    expect(mockApi.discoverFeed).toHaveBeenCalledWith('example.com');
    expect(mockApi.addFeed).toHaveBeenCalledWith('https://example.com/feed.xml', 'Example Blog', 'https://example.com', null);
    expect(app.addFeedOpen, 'the dialog closes after a successful add').toBe(false);
    expect(app.toasts.at(-1)?.message).toBe('Added Example Blog');
    expect(app.selection).toEqual({ kind: 'feed', id: 9 });
  });

  it('offers every discovered candidate when there are several', async () => {
    mockApi.discoverFeed.mockResolvedValue({
      kind: 'choose',
      candidates: [
        { url: 'https://example.com/atom.xml', title: 'Example (Atom)' },
        { url: 'https://example.com/rss.xml', title: null },
      ],
    });
    await typeAddress('example.com');
    await submit();

    const choices = [...document.querySelectorAll<HTMLButtonElement>('.choice')];
    expect(choices).toHaveLength(2);
    expect(choices[0]!.textContent).toContain('Example (Atom)');
    expect(choices[0]!.textContent).toContain('https://example.com/atom.xml');
    expect(choices[1]!.textContent).toContain('https://example.com/rss.xml');
    expect(app.addFeedOpen, 'still open until one is picked').toBe(true);

    choices[1]!.click();
    await settle();
    await settle();
    expect(mockApi.addFeed).toHaveBeenCalledWith('https://example.com/rss.xml', 'https://example.com/rss.xml', null, null);
    expect(app.addFeedOpen).toBe(false);
  });

  it('says what went wrong and stays open when nothing is found', async () => {
    mockApi.discoverFeed.mockRejectedValue({ kind: 'notFound', message: 'No Feed found at example.com — is the site up?' });
    await typeAddress('example.com');
    await submit();

    const alert = document.querySelector('[role="alert"]');
    expect(alert?.textContent).toBe('No Feed found at example.com — is the site up?');
    expect(app.addFeedOpen).toBe(true);
    expect(mockApi.addFeed).not.toHaveBeenCalled();
  });

  it('offers the Library\'s Folders and defaults to the top level', async () => {
    const select = document.getElementById('add-feed-folder') as HTMLSelectElement;
    expect([...select.options].map((o) => o.textContent)).toEqual(['No Folder (top level)', 'Tech']);
    // happy-dom doesn't implement :checked on options, so the binding can't be driven through
    // the DOM here; the pass-through of a chosen Folder id is covered by the state test below.
    mockApi.discoverFeed.mockResolvedValue({
      kind: 'found',
      url: 'https://example.com/feed.xml',
      title: 'Example Blog',
      siteUrl: null,
    });
    await typeAddress('example.com');
    await submit();
    expect(mockApi.addFeed).toHaveBeenCalledWith('https://example.com/feed.xml', 'Example Blog', null, null);
  });

  it('passes a chosen Folder id through to the Library', async () => {
    await app.addFeed('https://example.com/feed.xml', 'Example Blog', null, 3);
    expect(mockApi.addFeed).toHaveBeenCalledWith('https://example.com/feed.xml', 'Example Blog', null, 3);
  });

  it('an already-subscribed address selects the existing Feed instead', async () => {
    mockApi.discoverFeed.mockResolvedValue({
      kind: 'found',
      url: 'https://example.com/feed.xml',
      title: 'Example Blog',
      siteUrl: null,
    });
    mockApi.addFeed.mockResolvedValue({ kind: 'exists', feedId: 5 });
    await typeAddress('example.com');
    await submit();

    expect(app.toasts.at(-1)?.message).toBe('Already subscribed — showing that Feed');
    expect(app.selection).toEqual({ kind: 'feed', id: 5 });
    expect(app.addFeedOpen).toBe(false);
  });

  it('Cancel closes the dialog without touching the Library', async () => {
    const cancel = [...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent === 'Cancel')!;
    cancel.click();
    flushSync();
    expect(app.addFeedOpen).toBe(false);
    expect(mockApi.discoverFeed).not.toHaveBeenCalled();
  });
});
