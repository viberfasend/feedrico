// @vitest-environment happy-dom
// The virtualization primitive every Layout builds on (Henrik, The Archivist: 10k+ Articles
// must scroll at 60 fps). happy-dom reports clientHeight 0, which makes the render window
// deterministic: only the overscan band below the top is rendered.
import { createRawSnippet, flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ListQuery } from '../api';

const mockApi = {
  count: vi.fn(async (_q: ListQuery) => 0),
  list: vi.fn(async (_q: ListQuery, offset: number, limit: number) =>
    Array.from({ length: limit }, (_, i) => ({ id: offset + i })),
  ),
  sidebar: vi.fn(async () => ({ allUnread: 0, starred: 0, today: 0, folders: [], feeds: [] })),
};

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));

const { app } = await import('../state.svelte');
const { default: VirtualRows } = await import('./VirtualRows.svelte');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
// createRawSnippet receives the snippet's params as getters; call them inside render().
const rowSnippet = createRawSnippet((getIndex: () => number) => ({
  render: () => `<span>row-${getIndex()}</span>`,
}));

let mounted: ReturnType<typeof mount> | null = null;

function render(props: { rowHeight: number; perRow?: number; padding?: number; label?: string } = { rowHeight: 100 }) {
  mounted = mount(VirtualRows, {
    target: document.body,
    props: { label: 'Articles', row: rowSnippet, ...props },
  });
  flushSync();
  return document.body.querySelector('.viewport') as HTMLDivElement;
}

const rowTexts = () => [...document.body.querySelectorAll('.vrow')].map((r) => r.textContent);

beforeEach(() => {
  vi.clearAllMocks();
  app.total = 0;
  app.selectedIndex = null;
  app.listEpoch = 0;
});

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

describe('VirtualRows', () => {
  it('is a labelled listbox and renders only the overscan window, not all rows', async () => {
    app.total = 1000;
    const viewport = render({ rowHeight: 100 });
    await settle();
    flushSync();

    expect(viewport.getAttribute('role')).toBe('listbox');
    expect(viewport.getAttribute('aria-label')).toBe('Articles');
    // The spacer reserves the full height; only ~8 rows (800 px overscan / 100 px) exist.
    const spacer = viewport.querySelector('.spacer') as HTMLElement;
    expect(spacer.style.height).toBe('100000px');
    expect(rowTexts()).toEqual(['row-0', 'row-1', 'row-2', 'row-3', 'row-4', 'row-5', 'row-6', 'row-7']);
    // And it asked the Library for exactly the first page.
    expect(mockApi.list).toHaveBeenCalledTimes(1);
    expect(mockApi.list.mock.calls[0]![1]).toBe(0);
    expect(mockApi.list.mock.calls[0]![2]).toBe(100);
  });

  it('moves the window when scrolled and loads the pages it reveals', async () => {
    app.total = 1000;
    const viewport = render({ rowHeight: 100 });
    await settle();

    viewport.scrollTop = 50_000;
    viewport.dispatchEvent(new Event('scroll'));
    flushSync();
    await settle();

    expect(rowTexts()[0]).toBe('row-492'); // floor((50000 - 800) / 100)
    expect(rowTexts()).toHaveLength(16); // up to ceil((50000 + 800) / 100) = 508
    // The revealed rows cross page boundaries: pages 4 and 5 (offsets 400 and 500) load.
    const offsets = mockApi.list.mock.calls.map(([, offset]) => offset);
    expect(offsets).toContain(400);
    expect(offsets).toContain(500);
  });

  it('divides the total into rows of `perRow` Articles', () => {
    app.total = 100;
    render({ rowHeight: 100, perRow: 4 });
    const spacer = document.body.querySelector('.spacer') as HTMLElement;
    expect(spacer.style.height).toBe('2500px'); // ceil(100 / 4) rows × 100 px
  });

  it('scrolls back to the top when the list is reset (new View)', async () => {
    mockApi.count.mockResolvedValueOnce(1000);
    app.total = 1000;
    const viewport = render({ rowHeight: 100 });
    await settle();
    viewport.scrollTop = 5000;
    viewport.dispatchEvent(new Event('scroll'));
    flushSync();
    expect(viewport.scrollTop).toBe(5000);

    await app.resetList();
    flushSync();
    expect(viewport.scrollTop).toBe(0);
  });

  it('keeps the selected Article in view during keyboard navigation', () => {
    app.total = 1000;
    const viewport = render({ rowHeight: 100 });
    app.selectedIndex = 50;
    flushSync();
    // top of row 50 is 5000 px; with a 0 px viewport it scrolls so the row bottom is visible.
    expect(viewport.scrollTop).toBe(5100);
  });
});
