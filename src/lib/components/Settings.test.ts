// @vitest-environment happy-dom
// Settings dialog: opens from Ctrl/⌘+`, the sidebar More menu and the command palette; every
// change writes through to the Library (setSettings) and applies immediately; the refresh timer
// reschedules; the localStorage theme migrates into the Library once; Portable Mode disables
// start with OS.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ListQuery, Settings, Sidebar } from '../api';

const defaultSettings = (): Settings => ({
  appearance: { theme: 'system', density: 'comfortable' },
  reader: { fontFamily: 'serif', fontSize: 18, measure: 66, lineHeight: 1.65 },
  refresh: { intervalMinutes: 30 },
  imageCacheCapBytes: 1024 * 1024 * 1024,
  pruneReadDays: 90,
  pruneUnreadDays: 365,
});

let themeStoredInLibrary = false;
let lastSetSettings: Settings | null = null;

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
  setLayout: vi.fn(async () => {}),
  article: vi.fn(),
  setRead: vi.fn(async () => {}),
  deleteFolder: vi.fn(async () => {}),
  openExternal: vi.fn(async () => {}),
  checkForUpdates: vi.fn(),
  settings: vi.fn(async () => defaultSettings()),
  setSettings: vi.fn(async (s: Settings) => {
    lastSetSettings = s;
  }),
  hasSetting: vi.fn(async (key: string) => key === 'appearance.theme' && themeStoredInLibrary),
  librarySize: vi.fn(async () => 1_234_567),
  mediaSize: vi.fn(async () => 65_432),
  autostartEnabled: vi.fn(async () => true),
  setAutostart: vi.fn(async () => {}),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

const { app } = await import('../state.svelte');
const { default: App } = await import('../../App.svelte');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function press(key: string, init: KeyboardEventInit = {}) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
  flushSync();
}

let mounted: ReturnType<typeof mount> | null = null;

/** Mounts the app, waits out startup (which lands on the welcome screen), then enters ready. */
async function mountApp() {
  mounted = mount(App, { target: document.body });
  await settle();
  app.phase = 'ready';
  app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false };
  app.settings = defaultSettings();
  app.sidebar = { allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] };
  flushSync();
  await settle();
}

const dialog = () => document.querySelector('.settings-dialog');

/** Switches the Settings window to one of its sections. */
async function openSection(name: string) {
  const item = [...dialog()!.querySelectorAll<HTMLButtonElement>('[role="tab"]')].find(
    (el) => el.textContent?.trim() === name,
  )!;
  item.click();
  flushSync();
  await settle();
  flushSync();
}

async function openViaShortcut() {
  press(',', { ctrlKey: true });
  await settle();
  flushSync();
}

beforeEach(() => {
  vi.clearAllMocks();
  themeStoredInLibrary = false;
  lastSetSettings = null;
  localStorage.clear();
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.density;
  document.documentElement.removeAttribute('style');
  app.phase = 'ready';
  app.library = { path: '/lib', name: 'Lib', formatVersion: 3, feedCount: 1, portable: false };
  app.settings = defaultSettings();
  app.settingsOpen = false;
  app.addFeedOpen = false;
  app.sidebar = { allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] };
  app.total = 0;
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
  app.settingsOpen = false;
});

describe('Settings dialog entry points', () => {
  it('opens on Ctrl/⌘+, and again closes it', async () => {
    await mountApp();
    await openViaShortcut();
    expect(dialog(), 'Ctrl+, opens the Settings dialog').not.toBeNull();
    expect(dialog()?.textContent).toContain('Library');
    await openViaShortcut();
    expect(dialog(), 'Ctrl+, again closes it').toBeNull();
  });

  it('opens on ⌘+, too', async () => {
    await mountApp();
    press(',', { metaKey: true });
    await settle();
    flushSync();
    expect(dialog()).not.toBeNull();
  });

  it('opens from the sidebar More menu', async () => {
    await mountApp();
    const more = document.querySelector('button[aria-label="More actions"]') as HTMLElement;
    expect(more).not.toBeNull();
    more.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    flushSync();
    await settle();
    const item = [...document.querySelectorAll('[role="menuitem"]')].find((el) =>
      el.textContent?.includes('Settings'),
    ) as HTMLElement;
    expect(item, 'the More menu has a Settings entry').toBeDefined();
    item.click();
    await settle();
    flushSync();
    expect(dialog(), 'Settings… opens the dialog').not.toBeNull();
  });

  it('opens from the command palette action', async () => {
    await mountApp();
    press('k', { ctrlKey: true });
    await settle();
    flushSync();
    const input = document.getElementById('palette-input') as HTMLInputElement;
    input.value = 'settings';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    await settle();
    flushSync();
    expect(dialog(), 'the palette action opens Settings').not.toBeNull();
  });
});

describe('Settings dialog writes through to the Library', () => {
  it('changing the theme calls setSettings and sets data-theme', async () => {
    await mountApp();
    await openViaShortcut();
    const dark = dialog()!.querySelector<HTMLInputElement>('[aria-label="Theme"] input[value="dark"]')!;
    dark.click();
    await settle();
    flushSync();
    expect(mockApi.setSettings).toHaveBeenCalledTimes(1);
    expect(lastSetSettings?.appearance.theme).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('changing density sets data-density', async () => {
    await mountApp();
    await openViaShortcut();
    const compact = dialog()!.querySelector<HTMLInputElement>('[aria-label="Density"] input[value="compact"]')!;
    compact.click();
    await settle();
    flushSync();
    expect(lastSetSettings?.appearance.density).toBe('compact');
    expect(document.documentElement.dataset.density).toBe('compact');
  });

  it('hiding unread counts swaps numbers for dots and tells screen readers', async () => {
    await mountApp();
    app.sidebar = {
      allUnread: 7, starred: 2, today: 0, folders: [],
      feeds: [{ id: 1, title: 'Blog', siteUrl: null, unread: 7, lastError: null, lastSuccessAt: null, favicon: null }],
    };
    app.total = 7;
    flushSync();
    expect(document.querySelector('#pane-list .total')?.textContent).toBe('7');
    await app.updateSettings({ ...defaultSettings(), appearance: { theme: 'system', density: 'comfortable', unreadCounts: 'hide' } });
    flushSync();
    expect(document.documentElement.dataset.unreadCounts).toBe('hide');
    expect(lastSetSettings?.appearance.unreadCounts).toBe('hide');
    expect(document.querySelector('#pane-list .total')).toBeNull();
    const spoken = [...document.querySelectorAll('#pane-sidebar .count .sr-only')].map((e) => e.textContent);
    expect(spoken).toEqual(['has unread Articles', 'has unread Articles']);
  });

  it('changing the reader font size sets the reader custom properties on <html>', async () => {
    await mountApp();
    await openViaShortcut();
    await openSection('Reading');
    const size = dialog()!.querySelector('input[aria-label="Reader font size in pixels"]') as HTMLInputElement;
    size.value = '20';
    size.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    flushSync();
    expect(lastSetSettings?.reader.fontSize).toBe(20);
    expect(document.documentElement.style.getPropertyValue('--reader-font-size')).toBe('20px');
    expect(document.documentElement.style.getPropertyValue('--reader-measure')).toBe('66ch');
  });

  // Bug (review): a number outside its range (font size 30) was applied locally while the core
  // rejected it, and since every save writes all settings, each later change failed too.
  it('keeps reader numbers inside their range', async () => {
    await mountApp();
    await openViaShortcut();
    await openSection('Reading');
    const size = dialog()!.querySelector<HTMLInputElement>('input[aria-label="Reader font size in pixels"]')!;
    size.value = '30';
    size.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    flushSync();
    expect(lastSetSettings?.reader.fontSize).toBe(24);
    expect(size.value).toBe('24');
  });

  it('previews slider values while dragging and saves only on release', async () => {
    await mountApp();
    await openViaShortcut();
    await openSection('Reading');
    const size = dialog()!.querySelector<HTMLInputElement>('input[aria-label="Reader font size in pixels"]')!;
    size.value = '22';
    size.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(mockApi.setSettings).not.toHaveBeenCalled();
    expect(dialog()!.querySelector<HTMLElement>('.page-text')!.style.fontSize).toBe('22px');
    size.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(lastSetSettings?.reader.fontSize).toBe(22);
  });

  it('rolls back to the stored settings when saving fails', async () => {
    await mountApp();
    mockApi.setSettings.mockRejectedValueOnce({ kind: 'invalidSetting', message: 'nope' });
    await app.updateSettings({ ...defaultSettings(), appearance: { theme: 'dark', density: 'comfortable' } });
    flushSync();
    expect(app.settings?.appearance.theme).toBe('system');
    expect(document.documentElement.dataset.theme).not.toBe('dark');
  });

  it('reschedules the auto-refresh timer when the interval changes', async () => {
    const setIntervalSpy = vi.spyOn(globalThis, 'setInterval');
    await mountApp();
    await openViaShortcut();
    await openSection('Refresh and storage');
    const interval = [...dialog()!.querySelectorAll('select')].find((el) =>
      [...el.options].some((o) => o.value === '60'),
    )!;
    interval.value = '60';
    interval.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    flushSync();
    expect(lastSetSettings?.refresh.intervalMinutes).toBe(60);
    expect(setIntervalSpy).toHaveBeenCalledWith(expect.any(Function), 60 * 60 * 1000);
    setIntervalSpy.mockRestore();
  });

  it('shows Library location, size and format version', async () => {
    await mountApp();
    await openViaShortcut();
    await openSection('About this Library');
    const text = dialog()!.textContent ?? '';
    expect(text).toContain('/lib');
    expect(text).toContain('Format version');
    expect(text).toContain('3');
  });
});

describe('theme migration', () => {
  it('moves a stored localStorage theme into the Library once and clears the key', async () => {
    localStorage.setItem('feedrico.theme', 'dark');
    themeStoredInLibrary = false;
    await app.migrateStoredTheme();
    expect(lastSetSettings?.appearance.theme).toBe('dark');
    expect(localStorage.getItem('feedrico.theme')).toBeNull();
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('does nothing when the Library already has a theme stored', async () => {
    localStorage.setItem('feedrico.theme', 'dark');
    themeStoredInLibrary = true;
    await app.migrateStoredTheme();
    expect(mockApi.setSettings).not.toHaveBeenCalled();
    expect(localStorage.getItem('feedrico.theme')).toBeNull();
  });
});

describe('start with OS', () => {
  async function openMachineTab() {
    await openViaShortcut();
    const tab = [...dialog()!.querySelectorAll('button')].find((el) =>
      el.textContent?.trim() === 'This computer',
    ) as HTMLElement;
    tab.click();
    flushSync();
    await settle();
  }

  it('is enabled when not in Portable Mode and calls setAutostart on toggle', async () => {
    await mountApp();
    await openMachineTab();
    const toggle = [...dialog()!.querySelectorAll('button')].find((el) =>
      el.textContent?.includes('Start with OS'),
    ) as HTMLButtonElement;
    expect(toggle.disabled).toBe(false);
    toggle.click();
    await settle();
    expect(mockApi.setAutostart).toHaveBeenCalledWith(false);
  });

  it('is disabled with a hint in Portable Mode', async () => {
    await mountApp();
    app.library = { path: '/stick/Feedrico Library', name: 'Feedrico Library', formatVersion: 3, feedCount: 5, portable: true };
    flushSync();
    await openMachineTab();
    const toggle = [...dialog()!.querySelectorAll('button')].find((el) =>
      el.textContent?.includes('Start with OS'),
    ) as HTMLButtonElement;
    expect(toggle.disabled).toBe(true);
    expect(dialog()!.textContent).toContain('Not available in Portable Mode');
  });
});
