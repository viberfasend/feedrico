// @vitest-environment happy-dom
// Toasts: transient status messages in a polite live region. A message with an action (Undo,
// Open) lives longer and runs the action before dismissing; plain messages dismiss themselves.
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
  sidebar: vi.fn(async (): Promise<Sidebar> => ({ allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] })),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

const { app } = await import('../state.svelte');
const { default: Toasts } = await import('./Toasts.svelte');

let mounted: ReturnType<typeof mount> | null = null;

beforeEach(() => {
  app.toasts = [];
  mounted = mount(Toasts, { target: document.body });
  flushSync();
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  app.toasts = [];
  document.body.innerHTML = '';
});

describe('Toasts', () => {
  it('renders a polite live region that announces new messages', () => {
    const region = document.body.querySelector('.toasts')!;
    expect(region.getAttribute('role')).toBe('status');
    expect(region.getAttribute('aria-live')).toBe('polite');

    app.notify('3 marked as read');
    flushSync();
    expect(region.textContent).toContain('3 marked as read');
  });

  it('dismisses a plain message on its own after a few seconds', () => {
    vi.useFakeTimers();
    try {
      app.notify('hello');
      flushSync();
      expect(document.body.querySelectorAll('.toast')).toHaveLength(1);
      vi.advanceTimersByTime(4000);
      flushSync();
      expect(document.body.querySelectorAll('.toast')).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it('keeps a message with an action longer and runs the action before dismissing', () => {
    vi.useFakeTimers();
    try {
      const run = vi.fn();
      app.notify('Feedrico 0.2.0 is available', { label: 'Open', run });
      flushSync();
      vi.advanceTimersByTime(4000);
      flushSync();
      expect(document.body.querySelectorAll('.toast'), 'still there at the plain timeout').toHaveLength(1);

      const button = [...document.body.querySelectorAll<HTMLButtonElement>('.toast button')].find(
        (b) => b.textContent === 'Open',
      )!;
      button.click();
      flushSync();
      expect(run).toHaveBeenCalledTimes(1);
      expect(document.body.querySelectorAll('.toast')).toHaveLength(0);

      vi.advanceTimersByTime(8000);
      flushSync();
    } finally {
      vi.useRealTimers();
    }
  });

  it('dismiss(id) removes exactly that toast', () => {
    app.notify('one');
    app.notify('two');
    flushSync();
    const ids = app.toasts.map((t) => t.id);
    app.dismiss(ids[0]!);
    flushSync();
    expect(app.toasts.map((t) => t.message)).toEqual(['two']);
    expect(document.body.textContent).not.toContain('one');
    expect(document.body.textContent).toContain('two');
  });
});
