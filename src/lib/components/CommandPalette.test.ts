// @vitest-environment happy-dom
// Command palette behaviour: Ctrl/⌘+K opens and closes it, Escape restores focus, Enter runs
// the highlighted entry, and typing in its input never triggers the reading shortcuts.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ListQuery, Sidebar } from '../api';

const mockApi = {
  startup: vi.fn(async () => ({ kind: 'welcome', defaultPath: '' })),
  recentLibraries: vi.fn(async () => [] as string[]),
  onRefreshProgress: vi.fn(async () => () => {}),
  onMediaCached: vi.fn(async () => () => {}),
  onArticlesPruned: vi.fn(async () => () => {}),
  count: vi.fn(async (_q: ListQuery) => 0),
  list: vi.fn(async (_q: ListQuery, _o: number, _l: number) => []),
  refresh: vi.fn(async () => true),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

const { app } = await import('../state.svelte');
const { default: App } = await import('../../App.svelte');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

const sidebar: Sidebar = {
  allUnread: 3,
  starred: 0,
  today: 1,
  folders: [{ id: 2, name: 'Tech', unread: 2, feeds: [{ id: 5, title: 'Rust Blog', siteUrl: 'https://blog.rust-lang.org', unread: 2, lastError: null, lastSuccessAt: null, favicon: null }] }],
  feeds: [{ id: 7, title: 'Hacker News', siteUrl: 'https://news.ycombinator.com', unread: 1, lastError: null, lastSuccessAt: null, favicon: null }],
};

function press(key: string, init: KeyboardEventInit = {}) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
  flushSync();
}

async function openPalette() {
  press('k', { ctrlKey: true });
  await settle();
  flushSync();
  expect(document.querySelector('.palette-dialog [role="combobox"]'), 'palette is open').not.toBeNull();
  return document.getElementById('palette-input') as HTMLInputElement;
}

let mounted: ReturnType<typeof mount> | null = null;

function mountApp() {
  mounted = mount(App, { target: document.body });
}

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

beforeEach(() => {
  vi.clearAllMocks();
  app.sidebar = sidebar;
  app.phase = 'ready';
  app.selection = { kind: 'allUnread' };
  app.selectedIndex = null;
  app.addFeedOpen = false;
  app.total = 0;
});

describe('Command palette', () => {
  it('opens on Ctrl+K and closes on Ctrl+K again', async () => {
    mountApp();
    await settle();
    press('k', { ctrlKey: true });
    await settle();
    flushSync();
    expect(document.querySelector('[role="combobox"]'), 'Ctrl+K opens the palette').not.toBeNull();
    press('k', { ctrlKey: true });
    await settle();
    flushSync();
    expect(document.querySelector('[role="combobox"]'), 'Ctrl+K again closes it').toBeNull();
  });

  it('opens on ⌘+K too', async () => {
    mountApp();
    await settle();
    press('k', { metaKey: true });
    await settle();
    flushSync();
    expect(document.querySelector('[role="combobox"]')).not.toBeNull();
  });

  it('closes on Escape and returns focus to where it was', async () => {
    mountApp();
    await settle();
    const before = document.createElement('button');
    document.body.appendChild(before);
    before.focus();
    const input = await openPalette();
    expect(document.activeElement).toBe(input);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    flushSync();
    await settle();
    flushSync();
    expect(document.querySelector('[role="combobox"]'), 'Escape closes the palette').toBeNull();
    expect(document.activeElement, 'focus is restored').toBe(before);
  });

  it('Enter on a Feed switches the View', async () => {
    mountApp();
    await settle();
    const input = await openPalette();
    input.value = 'hacker';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    await settle();
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    flushSync();
    await settle();
    flushSync();
    expect(app.selection).toEqual({ kind: 'feed', id: 7 });
    expect(document.querySelector('[role="combobox"]'), 'palette closed after running').toBeNull();
  });

  it('Enter on the first action runs Refresh once', async () => {
    const spy = vi.spyOn(app, 'startRefresh').mockResolvedValue();
    mountApp();
    await settle();
    const input = await openPalette();
    expect(document.querySelector('[role="option"]')?.textContent).toContain('Refresh all Feeds');
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    flushSync();
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy).toHaveBeenCalledWith(true);
    spy.mockRestore();
  });

  it('arrow keys move the highlight and wrap around', async () => {
    mountApp();
    await settle();
    const input = await openPalette();
    const options = () => [...document.querySelectorAll('[role="option"]')] as HTMLElement[];
    const count = options().length;
    expect(count).toBeGreaterThan(3);
    expect(input.getAttribute('aria-activedescendant')).toBe('palette-option-0');
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
    flushSync();
    expect(input.getAttribute('aria-activedescendant')).toBe('palette-option-1');
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true, cancelable: true }));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true, cancelable: true }));
    flushSync();
    expect(input.getAttribute('aria-activedescendant')).toBe(`palette-option-${count - 1}`);
  });

  it('typing "j" in the palette input does not move the Article selection', async () => {
    const move = vi.spyOn(app, 'move');
    mountApp();
    await settle();
    app.total = 10;
    const input = await openPalette();
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'j', bubbles: true, cancelable: true }));
    flushSync();
    expect(move).not.toHaveBeenCalled();
    expect(app.selectedIndex).toBeNull();
    move.mockRestore();
  });

  it('does not open while the Add Feed dialog is open', async () => {
    mountApp();
    await settle();
    app.addFeedOpen = true;
    flushSync();
    await settle();
    press('k', { ctrlKey: true });
    await settle();
    flushSync();
    expect(document.querySelector('[role="combobox"]')).toBeNull();
  });
});
