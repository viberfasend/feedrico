// Folder suggestions (#123): the Assistant proposes which Folder each unsorted Feed could go
// in. The Feeds are sent in batches of BATCH_SIZE, so a Library of 1,035 Feeds (the Archivist)
// still works; the preview dialog shows the progress and a Stop button. The user accepts or
// rejects each proposed move, and applying is one undoable step — a toast with Undo, exactly
// like "mark all as read".
import { api, isCommandError, type FolderMove } from './api';
import {
  assistantNames,
  runAssistant,
  type AssistantAction,
  type AssistantErrorKind,
  type AssistantResult,
  type FolderSuggestion,
  type RunningTask,
} from './assistant';
import { assistantErrorKinds, assist, describeError } from './assistant-state.svelte';
import { app, errorMessage } from './state.svelte';

/** Mirrors BATCH_SIZE in crates/feedrico-core/src/assistant/folder_suggestions.rs. */
export const BATCH_SIZE = 100;

/** What is being sorted: the top-level Feeds, or the Feeds of one Folder. */
export type SuggestionScope = { kind: 'topLevel' } | { kind: 'folder'; id: number; name: string };

export interface Proposal {
  feedId: number;
  feedTitle: string;
  folder: string;
  /** The Folder doesn't exist yet; applying creates it. */
  isNewFolder: boolean;
  accepted: boolean;
}

export type Phase = 'running' | 'done' | 'stopped' | 'error';

/** Split `items` into batches of at most `size`, in order. */
export function planBatches<T>(items: readonly T[], size: number): T[][] {
  if (size < 1) throw new RangeError('batch size must be at least 1');
  const out: T[][] = [];
  for (let i = 0; i < items.length; i += size) out.push(items.slice(i, i + size));
  return out;
}

interface FeedRef {
  id: number;
  title: string;
}

class FolderSuggestionsState {
  /** The preview dialog. */
  open = $state(false);
  phase = $state<Phase | null>(null);
  scope = $state<SuggestionScope | null>(null);
  proposals = $state<Proposal[]>([]);
  totalFeeds = $state(0);
  /** Feeds already asked about. */
  doneFeeds = $state(0);
  batch = $state(0);
  batches = $state(0);
  error = $state<AssistantErrorKind | null>(null);
  /** What the polite live region says: progress and completion, never tokens. */
  announcement = $state('');
  applying = $state(false);
  private task: RunningTask | null = null;
  private seq = 0;

  get running() {
    return this.phase === 'running';
  }

  get acceptedCount() {
    return this.proposals.filter((p) => p.accepted).length;
  }

  get assistantName() {
    return assist.setup ? assistantNames[assist.setup.kind] : '';
  }

  /** The Feeds of the scope, from the sidebar: the top-level Feeds, or one Folder's Feeds. */
  private feedsOf(scope: SuggestionScope): FeedRef[] {
    const sidebar = app.sidebar;
    if (!sidebar) return [];
    if (scope.kind === 'topLevel') return sidebar.feeds.map((f) => ({ id: f.id, title: f.title }));
    return (sidebar.folders.find((f) => f.id === scope.id)?.feeds ?? []).map((f) => ({ id: f.id, title: f.title }));
  }

  /** One explicit user action: ask the Assistant about the scope's Feeds, batch by batch. */
  start(scope: SuggestionScope) {
    if (!assist.setup?.disclosureAccepted) return;
    this.seq++;
    const seq = this.seq;
    const feeds = this.feedsOf(scope);
    this.scope = scope;
    this.proposals = [];
    this.error = null;
    this.totalFeeds = feeds.length;
    this.doneFeeds = 0;
    this.batches = planBatches(feeds, BATCH_SIZE).length;
    this.batch = 0;
    this.announcement = '';
    this.open = true;
    if (!feeds.length) {
      this.phase = 'done';
      this.announcement = 'Nothing to sort';
      return;
    }
    this.phase = 'running';
    void this.run(feeds, seq);
  }

  private async run(feeds: FeedRef[], seq: number) {
    const scope = this.scope;
    if (!scope) return;
    const batches = planBatches(feeds, BATCH_SIZE);
    const byId = new Map(feeds.map((f) => [f.id, f]));
    const existing = new Set((app.sidebar?.folders ?? []).map((f) => f.name.toLowerCase()));
    const newFolders: string[] = [];
    for (let i = 0; i < batches.length; i++) {
      if (this.seq !== seq) return;
      this.batch = i + 1;
      this.doneFeeds = i * BATCH_SIZE;
      const action: AssistantAction = {
        kind: 'folderSuggestions',
        feedIds: batches[i]!.map((f) => f.id),
        newFolders: [...newFolders],
        folderId: scope.kind === 'folder' ? scope.id : null,
      };
      let result: AssistantResult;
      try {
        this.task = runAssistant(action, () => {});
        result = await this.task.result;
      } catch (e) {
        this.task = null;
        if (this.seq !== seq) return;
        const kind: AssistantErrorKind =
          isCommandError(e) && assistantErrorKinds.includes(e.kind as AssistantErrorKind)
            ? (e.kind as AssistantErrorKind)
            : 'failed';
        if (kind === 'cancelled') {
          this.phase = 'stopped';
          this.announcement = 'Stopped';
        } else {
          this.phase = 'error';
          this.error = kind;
          this.announcement = describeError(kind, assist.setup?.kind ?? 'claudeCode').what;
        }
        return;
      }
      this.task = null;
      if (this.seq !== seq) return;
      const moves = (result.structured as { moves?: FolderSuggestion[] } | null)?.moves ?? [];
      for (const m of moves) {
        const feed = byId.get(m.feedId);
        if (!feed || this.proposals.some((p) => p.feedId === m.feedId)) continue;
        const lower = m.folder.toLowerCase();
        const isNewFolder = !existing.has(lower) && !newFolders.some((n) => n.toLowerCase() === lower);
        if (isNewFolder) newFolders.push(m.folder);
        this.proposals = [
          ...this.proposals,
          { feedId: m.feedId, feedTitle: feed.title, folder: m.folder, isNewFolder, accepted: true },
        ];
      }
      this.doneFeeds = Math.min((i + 1) * BATCH_SIZE, feeds.length);
      this.announcement =
        i + 1 === batches.length ? 'Suggestions ready' : `Group ${i + 1} of ${batches.length} done`;
    }
    if (this.seq !== seq) return;
    this.phase = 'done';
    if (!this.proposals.length) this.announcement = 'No suggestions';
  }

  /** Stop: ends the current batch's process; the suggestions so far stay. */
  stop() {
    const task = this.task;
    this.task = null;
    task?.cancel().catch(() => {});
  }

  /** Accept or reject one proposed move. */
  toggle(feedId: number) {
    this.proposals = this.proposals.map((p) => (p.feedId === feedId ? { ...p, accepted: !p.accepted } : p));
  }

  /** Accept or reject every proposed move at once. */
  setAll(accepted: boolean) {
    this.proposals = this.proposals.map((p) => ({ ...p, accepted }));
  }

  /** Applies the accepted moves as one step; the toast's Undo reverses exactly that. */
  async apply() {
    const scope = this.scope;
    const accepted = this.proposals.filter((p) => p.accepted);
    if (!scope || !accepted.length || this.applying) return;
    this.applying = true;
    try {
      const moves: FolderMove[] = accepted.map((p) => ({ feedId: p.feedId, folder: p.folder }));
      const applied = await api.applyFolderMoves(moves, scope.kind === 'folder' ? scope.id : null);
      await app.loadSidebar();
      this.close();
      const n = applied.moved.length;
      app.notify(n === 1 ? '1 Feed moved' : `${n} Feeds moved`, {
        label: 'Undo',
        run: async () => {
          await api.undoFolderMoves(applied);
          await app.loadSidebar();
        },
      });
    } catch (e) {
      app.notify(errorMessage(e));
    } finally {
      this.applying = false;
    }
  }

  /** Close the dialog; a running batch is cancelled and its result thrown away. */
  close() {
    this.seq++;
    this.stop();
    this.open = false;
    this.phase = null;
    this.scope = null;
    this.proposals = [];
    this.announcement = '';
  }

  /** For tests: back to the initial state. */
  reset() {
    this.close();
    this.error = null;
    this.applying = false;
    this.totalFeeds = 0;
    this.doneFeeds = 0;
    this.batch = 0;
    this.batches = 0;
  }
}

export const folderSuggestions = new FolderSuggestionsState();
