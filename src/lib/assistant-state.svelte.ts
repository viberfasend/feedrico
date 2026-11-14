// The current Assistant run, shown in the reader's panel (#115). Results are ephemeral: one run
// at a time, never persisted, discarded when the Article changes or the reader closes.
//
// Feature issues (#116 and later) add an action to `assistantEntries`; the menu, the command
// palette and the panel pick it up from there.
import { isCommandError } from './api';
import { renderMarkdown } from './assistant-markdown';
import {
  assistantNames,
  assistantProviders,
  getAssistantSetup,
  runAssistant,
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
  action: AssistantAction;
}

export const assistantEntries: AssistantEntry[] = [
  // Proves the pipeline end to end. Easy to replace once real reading aids exist.
  {
    id: 'test',
    menuLabel: 'Check it works',
    paletteLabel: 'Assistant: Check it works',
    title: 'Check it works',
    action: { kind: 'test' },
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

  private seq = 0;
  private task: RunningTask | null = null;

  /** Reads the setup; cheap and local. Call again after the user changes it in Settings. */
  async loadSetup() {
    try {
      this.setup = await getAssistantSetup();
    } catch {
      this.setup = null;
    }
    if (!this.setup) this.discard();
  }

  /** Whether Assistant entry points apply right now: set up, with an Article in the reader. */
  get available() {
    return this.setup?.disclosureAccepted === true && app.article !== null && (!app.isGrid || app.readerOpen);
  }

  start(entry: AssistantEntry) {
    if (!this.setup?.disclosureAccepted) return;
    this.discard();
    const id = ++this.seq;
    this.run = { id, entry, phase: 'running', text: '', html: '', error: null };
    this.announcement = '';
    const live = () => (this.seq === id && this.run?.phase === 'running' ? this.run : null);
    const task = runAssistant(entry.action, (event) => {
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
