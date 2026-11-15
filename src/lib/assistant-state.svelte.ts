// The current Assistant run, shown in the reader's panel (#115) or, for View actions, above the
// Article list (#120/#121). Results are ephemeral: one run per scope at a time, never persisted,
// discarded when the Article (reader runs) or the View (View runs) changes.
//
// Feature issues add an action to `assistantEntries`; the menu, the command palette and the
// panels pick it up from there. Entries default to the reader's scope; 'view' entries act on the
// current View and need no open Article.
import { isCommandError } from './api';
import { guessLanguage, primarySubtag } from './assistant-language';
import { renderMarkdown } from './assistant-markdown';
import { readerSelection, type ReaderSelection } from './reader-selection';
import {
  assistantNames,
  assistantProviders,
  getAssistantSetup,
  parseSearchQueryResult,
  isPicksResult,
  isLocalAssistant,
  runAssistant,
  type ArticleNotes,
  type AssistantAction,
  type AssistantErrorKind,
  type AssistantKind,
  type AssistantResult,
  type AssistantSetup,
  type PicksResult,
  type RunningTask,
} from './assistant';
import { app } from './state.svelte';

export interface AssistantEntry {
  id: string;
  /** The item in the Assistant menu. */
  menuLabel: string;
  /** The command palette entry. */
  paletteLabel: string;
  /** Names the run in the panel. */
  title: string;
  /** Where the entry is offered: the reader ('reader', the default) or the current View ('view'). */
  scope?: 'reader' | 'view';
  /** Reader actions build their action from the open Article's id when the run starts; View
   *  actions ignore it. */
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
  {
    id: 'translate',
    menuLabel: 'Translate',
    paletteLabel: 'Assistant: Translate the Article',
    title: 'Translate',
    action: (articleId) => ({ kind: 'translate', articleId }),
  },
  {
    id: 'ask',
    menuLabel: 'Ask about this Article',
    paletteLabel: 'Assistant: Ask about this Article',
    title: 'Ask about this Article',
    action: (articleId) => ({ kind: 'ask', articleId, question: '', history: [] }),
  },
  {
    id: 'briefing',
    scope: 'view',
    menuLabel: 'Brief this View',
    paletteLabel: 'Assistant: Brief this View',
    title: 'Briefing',
    action: () => ({ kind: 'briefing', query: app.query() }),
  },
  {
    id: 'picks',
    scope: 'view',
    menuLabel: 'Picks for this View',
    paletteLabel: 'Assistant: Picks for this View',
    title: 'Picks',
    action: () => ({ kind: 'picks', query: app.query() }),
  },
];

export type RunPhase = 'running' | 'done' | 'stopped' | 'error';

export interface AssistantRun {
  id: number;
  entry: AssistantEntry;
  phase: RunPhase;
  /** Streamed text while running; the final text once done. */
  text: string;
  /** Sanitized HTML of the final text (the core's HTML for Translate, Markdown-rendered otherwise). */
  html: string;
  error: AssistantErrorKind | null;
  /** What the input was: excerpt, truncation, dropped turns. */
  notes: ArticleNotes | null;
  /** The validated structured result of a View run (#120/#121). */
  structured: unknown | null;
}

/** One earlier question and its answer in the Ask conversation (ephemeral, in memory only). */
export interface AskTurn {
  question: string;
  answer: string;
  html: string;
}

const BINARY: Record<AssistantKind, string> = {
  claudeCode: 'claude',
  mistralVibe: 'vibe',
  codex: 'codex',
  gemini: 'gemini',
  ollama: 'ollama',
};
const KINDS: AssistantErrorKind[] = ['notInstalled', 'notSignedIn', 'usageLimit', 'offline', 'timedOut', 'cancelled', 'malformed', 'failed', 'notSetUp', 'noInterests', 'invalid'];

/** Plain words for an error and what to do next. */
export function describeError(kind: AssistantErrorKind, assistant: AssistantKind): { what: string; next: string } {
  const name = assistantNames[assistant];
  const local = isLocalAssistant(assistant);
  switch (kind) {
    case 'notInstalled':
      return local
        ? { what: 'The Ollama server is not running on this computer.', next: 'Start Ollama, then try again.' }
        : { what: `${name} can't be found on this computer.`, next: 'Install it again, or choose where it is in Settings.' };
    case 'notSignedIn':
      return local
        ? { what: 'The Ollama server is not running on this computer.', next: 'Start Ollama, then try again.' }
        : { what: `${name} isn't signed in.`, next: `Open a terminal and run \`${BINARY[assistant]}\` to sign in, then try again.` };
    case 'usageLimit':
      return { what: `${name} says you've reached its usage limit.`, next: 'Wait until the limit resets, then try again.' };
    case 'offline':
      return local
        ? { what: "Couldn't reach the Ollama server on this computer.", next: 'Check that Ollama is running, then try again.' }
        : { what: `Couldn't reach ${assistantProviders[assistant]}.`, next: 'Check your internet connection, then try again.' };
    case 'timedOut':
      return { what: `${name} took too long to answer.`, next: 'Try again. A shorter Article may help.' };
    case 'cancelled':
      return { what: 'Stopped.', next: 'Try again whenever you like.' };
    case 'malformed':
      return { what: `${name} sent an answer Feedrico couldn't read.`, next: 'Try again.' };
    case 'notSetUp':
      return { what: 'No Assistant is set up on this computer.', next: 'Set one up in Settings, under Assistant.' };
    case 'noInterests':
      return { what: 'Picks need to know what you like to read.', next: 'Add a few sentences in Settings, under Assistant, Your interests.' };
    case 'invalid':
      return { what: 'That did not work for this View.', next: 'Try again, or choose another View.' };
    case 'failed':
      return { what: `${name} couldn't finish.`, next: `Try again. If it keeps failing, run \`${BINARY[assistant]}\` in a terminal to see what is wrong.` };
  }
}

/** Focus is in the reader pane, or nowhere in particular (the page body). */
function focusInReader(): boolean {
  const active = document.activeElement;
  return !active || active === document.body || !!document.getElementById('pane-reader')?.contains(active);
}

class AssistantState {
  /** The entries that apply right now (Translate only when the language differs). */
  get entries(): AssistantEntry[] {
    const target = primarySubtag(this.setup?.targetLanguage ?? navigator.language);
    const guessed = this.articleLanguage;
    return assistantEntries.filter(
      (e) =>
        e.id !== 'translate' ||
        (guessed !== null && target !== null && primarySubtag(guessed) !== target),
    );
  }
  /** The machine-local setup; null means Assistants are off and nothing of them is shown. */
  setup = $state<AssistantSetup | null>(null);
  /** The reader's Assistant menu, opened by the toolbar button or the shortcut. */
  menuOpen = $state(false);
  /** The Article list's Assistant menu for View actions (#120/#121). */
  viewMenuOpen = $state(false);
  run = $state<AssistantRun | null>(null);
  /** The Assistant search next to the search field (#122): idle, running or failed. */
  searchRun = $state<'idle' | 'running' | 'error'>('idle');
  searchError = $state<AssistantErrorKind | null>(null);
  /** The current View-level run (briefing, picks), shown above the Article list. */
  viewRun = $state<AssistantRun | null>(null);
  /** The picks of the last picks run, highlighted in the list for this session. */
  picks = $state<PicksResult | null>(null);
  /** What the polite live region says: completion, never tokens. */
  announcement = $state('');
  /** Ask mode (#119): the question field and the conversation so far, gone on Article change. */
  askOpen = $state(false);
  question = $state('');
  turns = $state<AskTurn[]>([]);
  /** How many oldest turns the last Ask run left out, so the panel can say so. */
  droppedTurns = $state(0);
  /** The reader's last text selection, input for "Explain a term" (#118). */
  selection = $state<ReaderSelection | null>(null);

  private listening = false;
  private seq = 0;
  private task: RunningTask | null = null;
  private lastQuestion = '';
  /** A cheap guess of the open Article's language (the Library stores no Feed language). */
  private articleLanguage = $derived(
    app.article ? guessLanguage(`${app.article.title}\n${app.article.contentHtml}`) : null,
  );
  private viewSeq = 0;
  private viewTask: RunningTask | null = null;

  /** Reads the setup; cheap and local. Call again after the user changes it in Settings. */
  async loadSetup() {
    try {
      this.setup = await getAssistantSetup();
    } catch {
      this.setup = null;
    }
    if (!this.setup) {
      this.discard();
      this.discardView();
    }
    // Keep the selection current while the user selects. Opening the palette or the menu moves
    // focus and collapses the live selection, so that keeps the last one; a selection cleared
    // while focus stays in the reader (a click on the text) forgets it.
    if (!this.listening && typeof document !== 'undefined') {
      this.listening = true;
      document.addEventListener('selectionchange', () => {
        const s = readerSelection();
        if (s) this.selection = s;
        else if (focusInReader()) this.selection = null;
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

  /** Whether the View-level entries apply: set up, with a Library ready. */
  get viewAvailable() {
    return this.setup?.disclosureAccepted === true && app.phase === 'ready';
  }

  /** The entries offered in the reader's Assistant menu. */
  get readerEntries() {
    return this.entries.filter((e) => (e.scope ?? 'reader') === 'reader');
  }

  /** The entries offered for the current View. */
  get viewEntries() {
    return assistantEntries.filter((e) => e.scope === 'view');
  }

  /** The pick for an Article id, if the last picks run ranked it. */
  pickFor(id: number) {
    return this.picks?.picks.find((p) => p.id === id) ?? null;
  }

  start(entry: AssistantEntry) {
    if (!this.setup?.disclosureAccepted) return;
    if ((entry.scope ?? 'reader') === 'view') {
      this.startView(entry);
      return;
    }
    if (entry.when && !entry.when()) return;
    // Article actions need the open Article; resolve the action before discard() clears state.
    const action =
      typeof entry.action === 'function' ? (app.article ? entry.action(app.article.id) : null) : entry.action;
    if (!action) return;
    this.discard();
    if (action.kind === 'ask') {
      // Ask opens the question field; each question is its own run.
      this.askOpen = true;
      return;
    }
    this.beginRun(entry, action);
  }

  /** Ask one question about the open Article; the conversation so far is resent with it (#119). */
  ask() {
    const question = this.question.trim();
    const article = app.article;
    if (!question || !this.setup?.disclosureAccepted || !article || this.run?.phase === 'running') return;
    const entry = assistantEntries.find((e) => e.id === 'ask')!;
    this.question = '';
    this.lastQuestion = question;
    this.beginRun(
      entry,
      {
        kind: 'ask',
        articleId: article.id,
        question,
        history: this.turns.map((t) => ({ question: t.question, answer: t.answer })),
      },
      {
        onDone: (result) => {
          this.turns = [
            ...this.turns,
            { question, answer: result.text, html: result.html ?? renderMarkdown(result.text) },
          ];
          this.droppedTurns = result.article?.droppedTurns ?? 0;
        },
        onError: () => {
          // The question goes back into the field, so a failed run can be sent again.
          this.question = question;
        },
      },
    );
  }

  private beginRun(
    entry: AssistantEntry,
    action: AssistantAction,
    hooks?: { onDone?: (result: AssistantResult) => void; onError?: () => void },
  ) {
    const id = ++this.seq;
    this.run = { id, entry, phase: 'running', text: '', html: '', error: null, notes: null, structured: null };
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
        // Translate's answer is HTML sanitized in the core; everything else is Markdown.
        run.html = result.html ?? renderMarkdown(result.text);
        run.notes = result.article ?? null;
        run.structured = result.structured;
        run.phase = 'done';
        this.announcement = 'Answer ready';
        hooks?.onDone?.(result);
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
        hooks?.onError?.();
      },
    );
  }

  /**
   * Ask the Assistant to turn `question` into a local search (#122). The validated query lands
   * in the search field, editable, and the search then runs locally as any search does. Only the
   * question goes to the Assistant, never Article content.
   */
  async askSearch(question: string): Promise<boolean> {
    if (!this.setup?.disclosureAccepted) return false;
    const q = question.trim();
    if (!q || this.searchRun === 'running') return false;
    this.searchRun = 'running';
    this.searchError = null;
    const task = runAssistant({ kind: 'searchQuery', question: q }, () => {});
    try {
      const result = await task.result;
      let parsed = parseSearchQueryResult(result.structured);
      if (!parsed) {
        try {
          parsed = parseSearchQueryResult(JSON.parse(result.text));
        } catch {
          parsed = null;
        }
      }
      if (!parsed) throw { kind: 'malformed', message: 'the answer was not a search' };
      void app.setSearchQuery(parsed.query, {
        feed: parsed.feed ?? undefined,
        folder: parsed.folder ?? undefined,
        since: parsed.since ?? undefined,
        until: parsed.until ?? undefined,
      });
      this.searchRun = 'idle';
      // Honest label: the Assistant wrote this query and it may be wrong; it is in the field.
      app.notify(`${assistantNames[this.setup.kind]} wrote this search — it may be wrong`);
      return true;
    } catch (e: unknown) {
      const kind =
        isCommandError(e) && KINDS.includes(e.kind as AssistantErrorKind) ? (e.kind as AssistantErrorKind) : 'failed';
      this.searchRun = 'error';
      this.searchError = kind;
      const d = describeError(kind, this.setup?.kind ?? 'claudeCode');
      app.notify(`${d.what} ${d.next}`);
      return false;
    }
  }

  /** A View-level run: briefing (#120) or picks (#121) of the current View. */
  private startView(entry: AssistantEntry) {
    this.discardView();
    const id = ++this.viewSeq;
    this.viewRun = { id, entry, phase: 'running', text: '', html: '', error: null, notes: null, structured: null };
    this.announcement = '';
    const live = () => (this.viewSeq === id && this.viewRun?.phase === 'running' ? this.viewRun : null);
    // View actions read the current View, not an Article; the id argument is unused.
    const action = typeof entry.action === 'function' ? entry.action(app.article?.id ?? 0) : entry.action;
    const task = runAssistant(action, (event) => {
      const run = live();
      if (run && event.kind === 'text') run.text += event.delta;
    });
    this.viewTask = task;
    task.result.then(
      (result) => {
        const run = live();
        if (!run) return;
        run.text = result.text;
        if (!result.structured) {
          run.phase = 'error';
          run.error = 'malformed';
          this.announcement = describeError('malformed', this.setup?.kind ?? 'claudeCode').what;
          return;
        }
        run.structured = result.structured;
        run.phase = 'done';
        if (entry.id === 'picks' && isPicksResult(result.structured)) this.picks = result.structured;
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

  /** Stop a View-level run the same way. */
  stopView() {
    const run = this.viewRun;
    if (!run || run.phase !== 'running') return;
    run.phase = 'stopped';
    this.cancelViewTask();
  }

  retry() {
    const run = this.run;
    if (!run) return;
    if (run.entry.id === 'ask') {
      // Re-ask the question of this run.
      this.question = this.lastQuestion;
      this.ask();
      return;
    }
    this.start(run.entry);
  }

  /** Close, or the Article changed: cancel a running task and throw the result and the
   *  conversation away. */
  discard() {
    this.seq++;
    if (this.run?.phase === 'running') this.cancelTask();
    this.task = null;
    this.run = null;
    this.announcement = '';
    this.askOpen = false;
    this.question = '';
    this.turns = [];
    this.droppedTurns = 0;
    this.lastQuestion = '';
    this.selection = null;
  }

  /** The View changed or the panel closed: cancel a View run and drop its result and picks. */
  discardView() {
    this.viewSeq++;
    if (this.viewRun?.phase === 'running') this.cancelViewTask();
    this.viewTask = null;
    this.viewRun = null;
    this.picks = null;
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
    this.discardView();
    this.setup = null;
    this.menuOpen = false;
    this.searchRun = 'idle';
    this.searchError = null;
    this.viewMenuOpen = false;
  }

  private cancelTask() {
    const task = this.task;
    this.task = null;
    task?.cancel().catch(() => {});
  }

  private cancelViewTask() {
    const task = this.viewTask;
    this.viewTask = null;
    task?.cancel().catch(() => {});
  }
}

export const assist = new AssistantState();

/** Every kind a run can fail with, so other Assistant surfaces can map a rejected invoke. */
export const assistantErrorKinds: readonly AssistantErrorKind[] = KINDS;
