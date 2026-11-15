// @vitest-environment happy-dom
// The Folder suggestions preview (#123): batches with progress and Stop, per-Feed accept/reject
// plus accept/reject all, apply as one undoable step (toast with Undo, like "mark all as read"),
// a Folder scope, and the off-by-default gate. Assistant runs are faked; no real CLI is used.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FeedNode, Sidebar } from '../api';
import type { AssistantAction, AssistantEvent, AssistantResult, AssistantSetup } from '../assistant';
import type { FolderSuggestion } from '../assistant';

const mockApi = {
  sidebar: vi.fn(async (): Promise<Sidebar> => sidebar),
  applyFolderMoves: vi.fn(async () => ({
    createdFolderIds: [9],
    moved: [
      { feedId: 1, previousFolderId: null, folderId: 9 },
      { feedId: 250, previousFolderId: null, folderId: 9 },
    ],
  })),
  undoFolderMoves: vi.fn(async () => {}),
};

interface FakeTask {
  action: AssistantAction;
  resolve: (r: AssistantResult) => void;
  reject: (e: unknown) => void;
  cancel: ReturnType<typeof vi.fn>;
}
const tasks: FakeTask[] = [];

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  api: mockApi,
}));
vi.mock('../assistant', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../assistant')>()),
  runAssistant: vi.fn((action: AssistantAction, _onEvent: (e: AssistantEvent) => void) => {
    let resolve!: FakeTask['resolve'];
    let reject!: FakeTask['reject'];
    const result = new Promise<AssistantResult>((res, rej) => ((resolve = res), (reject = rej)));
    result.catch(() => {});
    const cancel = vi.fn(async () => reject({ kind: 'cancelled', message: 'stopped' }));
    const task: FakeTask = { action, resolve, reject, cancel };
    tasks.push(task);
    return { runId: `r${tasks.length}`, result, cancel };
  }),
}));

const { app } = await import('../state.svelte');
const { assist } = await import('../assistant-state.svelte');
const { folderSuggestions, planBatches, BATCH_SIZE } = await import('../folder-suggestions.svelte');
const { default: FolderSuggestions } = await import('./FolderSuggestions.svelte');

const setup: AssistantSetup = { kind: 'claudeCode', path: '/bin/claude', model: null, disclosureAccepted: true };

const feedNode = (id: number, title = `Feed ${id}`): FeedNode => ({
  id,
  title,
  siteUrl: `https://${id}.example`,
  unread: 0,
  lastError: null,
  lastSuccessAt: null,
  favicon: null,
});

let sidebar: Sidebar;
function makeSidebar(topCount: number): Sidebar {
  return {
    allUnread: 0,
    starred: 0,
    today: 0,
    folders: [
      { id: 5, name: 'Messy', unread: 0, feeds: [feedNode(90, 'Messy One'), feedNode(91, 'Messy Two')] },
      { id: 6, name: 'Tech', unread: 0, feeds: [] },
    ],
    feeds: Array.from({ length: topCount }, (_, i) => feedNode(i + 1)),
  };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const qa = <T extends HTMLElement>(sel: string) => [...document.querySelectorAll<T>(sel)];
const button = (label: string) =>
  qa<HTMLButtonElement>('button').find((b) => b.textContent?.trim().startsWith(label));

let mounted: ReturnType<typeof mount> | null = null;

async function render(topCount: number) {
  sidebar = makeSidebar(topCount);
  app.sidebar = sidebar;
  assist.setup = setup;
  mounted = mount(FolderSuggestions, { target: document.body });
  await settle();
  flushSync();
}

/** Resolve the current batch with these moves and let the loop continue. */
async function resolveNext(moves: FolderSuggestion[]) {
  const t = tasks.at(-1);
  expect(t, 'a batch is running').toBeTruthy();
  t!.resolve({ text: '', structured: { moves } });
  await settle();
  flushSync();
}

beforeEach(() => {
  tasks.length = 0;
  vi.clearAllMocks();
  mockApi.sidebar.mockImplementation(async () => sidebar);
  vi.spyOn(app, 'notify').mockImplementation(() => {});
});

afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  folderSuggestions.reset();
  assist.reset();
  app.sidebar = null;
  document.body.innerHTML = '';
});

describe('planBatches', () => {
  it('splits in order and covers the Archivist’s 1,035 Feeds in 11 batches', () => {
    expect(planBatches([], BATCH_SIZE)).toEqual([]);
    expect(planBatches([1, 2], BATCH_SIZE)).toEqual([[1, 2]]);
    const big = Array.from({ length: 1035 }, (_, i) => i);
    const batches = planBatches(big, BATCH_SIZE);
    expect(batches).toHaveLength(11);
    expect(batches.slice(0, 10).every((b) => b.length === 100)).toBe(true);
    expect(batches[10]).toHaveLength(35);
    expect(batches.flat()).toEqual(big);
    expect(() => planBatches([1], 0)).toThrow(RangeError);
  });
});

describe('off by default', () => {
  it('runs nothing without an accepted setup', async () => {
    await render(5);
    assist.setup = null;
    folderSuggestions.start({ kind: 'topLevel' });
    flushSync();
    expect(folderSuggestions.open).toBe(false);
    expect(tasks).toHaveLength(0);
    expect(q('.folder-suggestions-dialog')).toBeNull();
  });
});

describe('a run over batches', () => {
  it('sends the top-level Feeds in batches of 100, with progress, and reuses new Folder names', async () => {
    await render(250);
    folderSuggestions.start({ kind: 'topLevel' });
    flushSync();
    expect(folderSuggestions.open).toBe(true);
    expect(folderSuggestions.phase).toBe('running');
    expect(folderSuggestions.batches).toBe(3);
    expect(q('.progress')!.textContent).toContain('group 1 of 3');
    expect(q('.progress')!.textContent).toContain('1–100 of 250 Feeds');
    expect(q('.label')!.textContent).toContain('Proposed by Claude Code · may be wrong');

    expect(tasks).toHaveLength(1);
    expect(tasks[0]!.action).toEqual({
      kind: 'folderSuggestions',
      feedIds: Array.from({ length: 100 }, (_, i) => i + 1),
      newFolders: [],
      folderId: null,
    });
    await resolveNext([
      { feedId: 1, folder: 'Tech' },
      { feedId: 2, folder: 'Cooking' },
    ]);
    expect(q('[aria-live="polite"]')!.textContent).toBe('Group 1 of 3 done');
    expect(tasks).toHaveLength(2);
    // The Folder name the first batch proposed is offered to the next one.
    expect((tasks[1]!.action as { newFolders: string[] }).newFolders).toEqual(['Cooking']);

    await resolveNext([]);
    expect(tasks).toHaveLength(3);
    await resolveNext([{ feedId: 250, folder: 'Cooking' }]);
    expect(folderSuggestions.phase).toBe('done');
    expect(q('[aria-live="polite"]')!.textContent).toBe('Suggestions ready');

    const boxes = qa<HTMLInputElement>('.moves input[type="checkbox"]');
    expect(boxes).toHaveLength(3);
    expect(boxes.every((b) => b.checked), 'all proposals start accepted').toBe(true);
    const text = q('.moves')!.textContent ?? '';
    expect(text).toContain('Feed 1');
    expect(text).toContain('Tech');
    expect(text).toContain('Cooking');
    // Cooking is marked new once; later batches reuse the name.
    expect(qa('.new')).toHaveLength(1);
  });

  it('says so when there is nothing to sort', async () => {
    await render(0);
    folderSuggestions.start({ kind: 'topLevel' });
    flushSync();
    expect(folderSuggestions.phase).toBe('done');
    expect(tasks).toHaveLength(0);
    expect(q('.empty')!.textContent).toContain('No suggestions');
  });

  it('Stop cancels the running batch and keeps the suggestions so far', async () => {
    await render(250);
    folderSuggestions.start({ kind: 'topLevel' });
    flushSync();
    await resolveNext([{ feedId: 1, folder: 'Tech' }]);
    expect(tasks).toHaveLength(2);
    button('Stop')!.click();
    await settle();
    flushSync();
    expect(tasks[1]!.cancel).toHaveBeenCalledTimes(1);
    expect(folderSuggestions.phase).toBe('stopped');
    expect(q('[aria-live="polite"]')!.textContent).toBe('Stopped');
    expect(folderSuggestions.proposals).toHaveLength(1);
    expect(button('Move 1 Feed')).not.toBeNull();
    expect(q('.progress')).toBeNull();
  });

  it('shows a plain-words error with a next step and can try again', async () => {
    await render(5);
    folderSuggestions.start({ kind: 'topLevel' });
    flushSync();
    tasks[0]!.reject({ kind: 'notSignedIn', message: 'raw' });
    await settle();
    flushSync();
    expect(folderSuggestions.phase).toBe('error');
    expect(q('.error')!.textContent).toContain("isn't signed in");
    expect(q('.error')!.textContent).toContain('run `claude`');
    button('Try again')!.click();
    await settle();
    flushSync();
    expect(tasks).toHaveLength(2);
  });

  it('sorts one chosen Folder and applies with that scope', async () => {
    await render(2);
    folderSuggestions.start({ kind: 'folder', id: 5, name: 'Messy' });
    flushSync();
    expect(q('.dialog-title')!.textContent).toBe('Suggest Folders for Messy');
    expect((tasks[0]!.action as { folderId: number | null }).folderId).toBe(5);
    expect((tasks[0]!.action as { feedIds: number[] }).feedIds).toEqual([90, 91]);
    await resolveNext([{ feedId: 90, folder: 'Tech' }]);
    expect(folderSuggestions.phase).toBe('done');
    button('Move 1 Feed')!.click();
    await settle();
    flushSync();
    expect(mockApi.applyFolderMoves).toHaveBeenCalledWith([{ feedId: 90, folder: 'Tech' }], 5);
  });
});

describe('the preview', () => {
  async function startWithThreeProposals() {
    await render(250);
    folderSuggestions.start({ kind: 'topLevel' });
    flushSync();
    await resolveNext([
      { feedId: 1, folder: 'Tech' },
      { feedId: 2, folder: 'Cooking' },
    ]);
    await resolveNext([]);
    await resolveNext([{ feedId: 250, folder: 'Cooking' }]);
    expect(folderSuggestions.phase).toBe('done');
    expect(folderSuggestions.proposals).toHaveLength(3);
  }

  it('accepts and rejects per Feed, and all at once', async () => {
    await startWithThreeProposals();
    const boxes = qa<HTMLInputElement>('.moves input[type="checkbox"]');
    // Reject one Feed: its checkbox is labelled with the move.
    expect(boxes[1]!.getAttribute('aria-label')).toBe('Move Feed 2 to Cooking');
    boxes[1]!.checked = false;
    boxes[1]!.dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    expect(folderSuggestions.acceptedCount).toBe(2);
    expect(q('.count')!.textContent).toBe('2 of 3 accepted');

    button('Reject all')!.click();
    flushSync();
    expect(folderSuggestions.acceptedCount).toBe(0);
    expect(button('Move 0 Feeds')!.disabled).toBe(true);

    button('Accept all')!.click();
    flushSync();
    expect(folderSuggestions.acceptedCount).toBe(3);
  });

  it('is keyboard operable: every control is a native focusable element', async () => {
    await startWithThreeProposals();
    const controls = qa<HTMLElement>('.folder-suggestions-dialog button, .folder-suggestions-dialog input');
    expect(controls.length).toBeGreaterThanOrEqual(5);
    for (const el of controls) expect(el.tabIndex).toBe(0);
    const box = qa<HTMLInputElement>('.moves input[type="checkbox"]')[0]!;
    box.focus();
    expect(document.activeElement).toBe(box);
    // Space on a checkbox is a click; happy-dom doesn't synthesize it, so click() stands in.
    box.click();
    flushSync();
    expect(folderSuggestions.proposals[0]!.accepted).toBe(false);
  });

  it('applies the accepted moves as one step and offers Undo in the toast', async () => {
    await startWithThreeProposals();
    const boxes = qa<HTMLInputElement>('.moves input[type="checkbox"]');
    boxes[1]!.checked = false;
    boxes[1]!.dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();

    button('Move 2 Feeds')!.click();
    await settle();
    flushSync();
    expect(mockApi.applyFolderMoves).toHaveBeenCalledWith(
      [
        { feedId: 1, folder: 'Tech' },
        { feedId: 250, folder: 'Cooking' },
      ],
      null,
    );
    // The dialog closed and the sidebar reloaded.
    expect(folderSuggestions.open).toBe(false);
    expect(q('.folder-suggestions-dialog')).toBeNull();
    expect(mockApi.sidebar).toHaveBeenCalled();

    // One toast, one Undo action that reverses exactly the applied step.
    const notify = vi.mocked(app.notify);
    expect(notify).toHaveBeenCalledTimes(1);
    const [message, action] = notify.mock.calls[0]!;
    expect(message).toBe('2 Feeds moved');
    expect(action?.label).toBe('Undo');
    const applied = await mockApi.applyFolderMoves.mock.results[0]!.value;
    await action!.run();
    expect(mockApi.undoFolderMoves).toHaveBeenCalledWith(applied);
  });
});
