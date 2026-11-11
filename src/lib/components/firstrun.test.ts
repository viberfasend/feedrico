// @vitest-environment happy-dom
// First run (#60): an empty Library offers one obvious first step in plain words, and the
// reader pane doesn't tell a Newcomer to pick an Article when there are none.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ListQuery, Sidebar } from '../api';

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '/data/Feedrico Library' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 0),
  list: vi.fn(async (_q: ListQuery, _o: number, _l: number) => []),
  refresh: vi.fn(async () => false),
  sidebar: vi.fn(async (): Promise<Sidebar> => ({ allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] })),
};
vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

const { app } = await import('../state.svelte');
const { default: App } = await import('../../App.svelte');
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

let mounted: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

describe('first run', () => {
  it('describes where the Library goes in plain words on the welcome screen', async () => {
    mounted = mount(App, { target: document.body });
    await settle();
    flushSync();
    const text = document.body.textContent ?? '';
    expect(text).toContain('app data folder');
    expect(text).not.toContain('/data/Feedrico Library');
    expect(document.querySelector('.where')?.getAttribute('title')).toBe('/data/Feedrico Library');
  });

  it('offers adding a website first and explains the import in an empty Library', async () => {
    mounted = mount(App, { target: document.body });
    await settle();
    app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 0, portable: false };
    app.sidebar = { allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] };
    app.phase = 'ready';
    flushSync();
    const list = document.getElementById('pane-list')!;
    const buttons = [...list.querySelectorAll('button')].map((b) => b.textContent?.trim());
    expect(buttons).toContain('Add a website or Feed…');
    expect(buttons).toContain('Import a list of Feeds…');
    expect(document.getElementById('pane-reader')?.textContent).not.toContain('Pick an Article');
    for (const button of document.querySelectorAll('#pane-sidebar .onboarding button')) {
      expect(button.classList.contains('btn'), 'sidebar onboarding uses the library Button').toBe(true);
    }
    expect(document.querySelectorAll('#pane-sidebar .onboarding button').length).toBe(2);
  });
});
