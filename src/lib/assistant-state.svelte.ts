// The current Assistant run, shown in the reader's panel (#115). Results are ephemeral: one run
// at a time, never persisted, discarded when the Article changes or the reader closes.
//
// Feature issues (#116 and later) add an action to `assistantEntries`; the menu, the command
// palette and the panel pick it up from there.
import { isCommandError } from './api';
import { renderMarkdown } from './assistant-markdown';
import { readerSelection, type ReaderSelection } from './reader-selection';
import {
  assistantNames,
  assistantProviders,
  getAssistantSetup,
  runAssistant,
  type ArticleNotes,
  type AssistantAction,
  type AssistantErrorKind,
  type AssistantKind,
  type AssistantSetup,
  type RunningTask,
} from './assistant';
import { app } from './state.svelte';

export interface AssistantEntry {
  id: string;
  /** The item in the reader's Assistant menu. */
  menuLabel: string;
  /** The command palette entry. */
  paletteLabel: string;
  /** Names the run in the panel. */
  title: string;
  /** Article actions build their action from the open Article's id when the run starts. */
  action: AssistantAction | ((articleId: number) => AssistantAction);
  /** Optional gate, re-checked whenever the menu or palette renders (a selection exists, …). */
  when?: () => boolean;
}

export const assistantEntries: AssistantEntry[] = [
  {
    id: 'summarize',
    menuLabel: 'Summarize (u)',
    paletteLabel: 'Assistant: Summarize this Article',
    title: 'Summary',
    action: (articleId) => ({ kind: 'summarize', articleId }),
  },
  {
    id: 'explain',
    menuLabel: 'Explain simply',
    paletteLabel: 'Assistant: Explain this Article in plain language',
    title: 'Plain-language explanation',
    action: (articleId) => ({ kind: 'explain', articleId }),
  },
  {
    id: 'explainTerm',
    menuLabel: 'Explain selection',
    paletteLabel: 'Assistant: Explain the selected term',
    title: 'Term explained',
    when: () => assist.currentSelection() !== null,
    action: (articleId) => {
      const s = assist.currentSelection() ?? { term: '', context: '' };
      return { kind: 'explainTerm', articleId, term: s.term, context: s.context };
    },
  },
];

export type RunPhase = 'running' | 'done' | 'stopped' | 'error';

export interface AssistantRun {
  id: number;
  entry: AssistantEntry;
  phase: RunPhase;
  /** Streamed text while running; the final text once done. */
  text: string;
  /** Sanitized HTML of the final text. */
  html: string;
  error: AssistantErrorKind | null;
  /** What the panel says about the input an Article result is based on. */
  article: ArticleNotes | null;
}

const BINARY: Record<AssistantKind, string> = { claudeCode: 'claude', mistralVibe: 'vibe' };
const KINDS: AssistantErrorKind[] = ['notInstalled', 'notSignedIn', 'usageLimit', 'offline', 'timedOut', 'cancelled', 'malformed', 'failed', 'notSetUp'];

/** Plain words for an error and what to do next. */
export function describeError(kind: AssistantErrorKind, assistant: AssistantKind): { what: string; next: string } {
  const name = assistantNames[assistant];
  switch (kind) {
    case 'notInstalled':
      return { what: `${name} can't be found on this computer.`, next: 'Install it again, or choose where it is in Settings.' };
    case 'notSignedIn':
      return { what: `${name} isn't signed in.`, next: `Open a terminal and run \`${BINARY[assistant]}\` to sign in, then try again.` };
    case 'usageLimit':
      return { what: `${name} says you've reached its usage limit.`, next: 'Wait until the limit resets, then try again.' };
    case 'offline':
      return { what: `Couldn't reach ${assistantProviders[assistant]}.`, next: 'Check your internet connection, then try again.' };
    case 'timedOut':
      return { what: `${name} took too long to answer.`, next: 'Try again. A shorter Article may help.' };
    case 'cancelled':
      return { what: 'Stopped.', next: 'Try again whenever you like.' };
    case 'malformed':
      return { what: `${name} sent an answer Feedrico couldn't read.`, next: 'Try again.' };
    case 'notSetUp':
      return { what: 'No Assistant is set up on this computer.', next: 'Set one up in Settings, under Assistant.' };
    case 'failed':
      return { what: `${name} couldn't finish.`, next: `Try again. If it keeps failing, run \`${BINARY[assistant]}\` in a terminal to see what is wrong.` };
  }
}

class AssistantState {
  readonly entries = assistantEntries;
  /** The machine-local setup; null means Assistants are off and nothing of them is shown. */
  setup = $state<AssistantSetup | null>(null);
  /** The reader's Assistant menu, opened by the toolbar button or the shortcut. */
  menuOpen = $state(false);
  run = $state<AssistantRun | null>(null);
  /** What the polite live region says: completion, never tokens. */
  announcement = $state('');
  /** The reader's last text selection, input for "Explain a term" (#118). */
  selection = $state<ReaderSelection | null>(null);

  private seq = 0;
  private task: RunningTask | null = null;
  private listening = false;

  /** Reads the setup; cheap and local. Call again after the user changes it in Settings. */
  async loadSetup() {
    try {
      this.setup = await getAssistantSetup();
    } catch {
      this.setup = null;
    }
    if (!this.setup) this.discard();
    // Keep the selection current while the user selects; a collapsed selection never clears it,
    // because opening the palette moves focus and collapses the live selection.
    if (!this.listening && typeof document !== 'undefined') {
      this.listening = true;
      document.addEventListener('selectionchange', () => {
        const s = readerSelection();
        if (s) this.selection = s;
      });
    }
  }

  /** Whether Assistant entry points apply right now: set up, with an Article in the reader. */
  get available() {
    return this.setup?.disclosureAccepted === true && app.article !== null && (!app.isGrid || app.readerOpen);
  }

  /** The selection to use now: the last one seen, else the live one. */
  currentSelection(): ReaderSelection | null {
    return this.selection ?? readerSelection();
  }

  /** Remember the reader's current selection, if any (called when a menu or palette opens). */
  captureSelection() {
    const s = readerSelection();
    if (s) this.selection = s;
  }

  /** Start an entry by id (the Summarize shortcut). */
  startEntry(id: string) {
    const entry = this.entries.find((e) => e.id === id);
    if (entry) this.start(entry);
  }

  start(entry: AssistantEntry) {
    if (!this.setup?.disclosureAccepted) return;
    if (entry.when && !entry.when()) return;
    // Article actions need the open Article; resolve the action before discard() clears state.
    const action =
      typeof entry.action === 'function'
        ? app.article
          ? entry.action(app.article.id)
          : null
        : entry.action;
    if (!action) return;
    this.discard();
    const id = ++this.seq;
    this.run = { id, entry, phase: 'running', text: '', html: '', error: null, article: null };
    this.announcement = '';
    const live = () => (this.seq === id && this.run?.phase === 'running' ? this.run : null);
    const task = runAssistant(action, (event) => {
      const run = live();
      if (run && event.kind === 'text') run.text += event.delta;
    });
    this.task = task;
    task.result.then(
      (result) => {
        const run = live();
        if (!run) return;
        run.text = result.text;
        run.html = renderMarkdown(result.text);
        run.article = result.article ?? null;
        run.phase = 'done';
        this.announcement = 'Answer ready';
      },
      (e: unknown) => {
        const run = live();
        if (!run) return;
        const kind = isCommandError(e) && KINDS.includes(e.kind as AssistantErrorKind) ? (e.kind as AssistantErrorKind) : 'failed';
        if (kind === 'cancelled') {
          run.phase = 'stopped';
          return;
        }
        run.phase = 'error';
        run.error = kind;
        this.announcement = describeError(kind, this.setup?.kind ?? 'claudeCode').what;
      },
    );
  }

  /** Stop: ends the process, keeps what already arrived. */
  stop() {
    const run = this.run;
    if (!run || run.phase !== 'running') return;
    run.phase = 'stopped';
    run.html = renderMarkdown(run.text);
    this.cancelTask();
  }

  retry() {
    const entry = this.run?.entry;
    if (entry) this.start(entry);
  }

  /** Close, or the Article changed: cancel a running task and throw the result away. */
  discard() {
    this.seq++;
    if (this.run?.phase === 'running') this.cancelTask();
    this.task = null;
    this.run = null;
    this.announcement = '';
    this.selection = null;
  }

  async copy() {
    const text = this.run?.text;
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      app.notify('Copied');
    } catch {
      app.notify("Couldn't copy to the clipboard");
    }
  }

  /** For tests: back to the initial state. */
  reset() {
    this.discard();
    this.setup = null;
    this.menuOpen = false;
  }

  private cancelTask() {
    const task = this.task;
    this.task = null;
    task?.cancel().catch(() => {});
  }
}

export const assist = new AssistantState();
