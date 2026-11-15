// Typed bridge for Assistants (epic #110). It's kept apart from api.ts so the Assistant issues
// don't collide with other work. The shape mirrors crates/feedrico-core/src/assistant/mod.rs.
//
// Shell commands (src-tauri) that implement this contract:
//   assistant_detect { manualPath }          -> FoundAssistant[]          (#112)
//   assistant_setup_get                      -> AssistantSetup | null     (#114)
//   assistant_setup_set { setup }            -> void                      (#114)
//   assistant_run { runId, action }          -> AssistantResult           (#113)
//     streams 'assistant-event' with { runId, event: AssistantEvent } while running;
//     rejects with a CommandError whose kind is an AssistantError kind, or 'notSetUp'
//     when there is no setup or its disclosure isn't accepted (then nothing runs)
//   assistant_cancel { runId }               -> void                      (#113)
//   apply_folder_moves { moves, scopeFolderId } -> AppliedMoves           (#123, in api.ts)
//   undo_folder_moves { applied }            -> void                      (#123, in api.ts)

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ListQuery } from './api';

export type AssistantKind = 'claudeCode' | 'mistralVibe' | 'codex' | 'gemini' | 'ollama';
export type SignIn = 'plan' | 'apiKey' | 'notSignedIn' | 'unknown' | 'notApplicable';

export interface FoundAssistant {
  kind: AssistantKind;
  path: string;
  version: string | null;
  signIn: SignIn;
  /** The models installed on this computer; only Ollama fills this. */
  models: string[];
}

/** Machine-local, never in the Library. `null` from assistant_setup_get means off. */
export interface AssistantSetup {
  kind: AssistantKind;
  path: string;
  model: string | null;
  /** The user has seen the disclosure that Article text goes to the provider. */
  disclosureAccepted: boolean;
  /** The language Translate renders Articles into; null/absent means the system language. */
  targetLanguage?: string | null;
  /** Free text: what the user likes to read; used only by the picks run (#121). */
  interests?: string;
}

/** What to run. Wave 1 has only 'test'; each feature issue adds its action. */
export type AssistantAction =
  | { kind: 'test' }
  | { kind: 'summarize'; articleId: number }
  | { kind: 'explain'; articleId: number }
  | { kind: 'explainTerm'; articleId: number; term: string; context: string }
  | { kind: 'translate'; articleId: number }
  | { kind: 'ask'; articleId: number; question: string; history: { question: string; answer: string }[] }
  | {
      /** One batch of Folder suggestions (#123). */
      kind: 'folderSuggestions';
      /** The Feed ids of one batch; the shell refuses more than its BATCH_SIZE at once. */
      feedIds: number[];
      /** Folder names earlier batches of the same run proposed, so the model reuses them. */
      newFolders: string[];
      /** The Folder whose Feeds are being sorted, or null for the top level. */
      folderId: number | null;
    }
  | { kind: 'searchQuery'; question: string }
  | { kind: 'briefing'; query: ListQuery }
  | { kind: 'picks'; query: ListQuery };

/** One proposed move from a 'folderSuggestions' run: the run's structured output. */
export interface FolderSuggestion {
  feedId: number;
  folder: string;
}

/** The validated briefing of a View (#120), as the shell returns it in `structured`. */
export interface BriefingResult {
  considered: number;
  capped: boolean;
  topics: { topic: string; sentence: string; articleIds: number[] }[];
  articles: { id: number; title: string; feedTitle: string }[];
}

/** The validated picks of a View (#121), as the shell returns it in `structured`. */
export interface PicksResult {
  considered: number;
  capped: boolean;
  picks: { id: number; title: string; feedTitle: string; score: number; reason: string }[];
}

export function isBriefingResult(v: unknown): v is BriefingResult {
  return typeof v === 'object' && v !== null && Array.isArray((v as BriefingResult).topics) && Array.isArray((v as BriefingResult).articles);
}

export function isPicksResult(v: unknown): v is PicksResult {
  return typeof v === 'object' && v !== null && typeof (v as PicksResult).considered === 'number' && Array.isArray((v as PicksResult).picks);
}

export type AssistantEvent =
  | { kind: 'text'; delta: string }
  | { kind: 'done'; text: string; structured: unknown | null };

/** What the panel says about the input an Article result is based on. */
export interface ArticleNotes {
  /** The Feed gave only a short excerpt, so the answer covers that excerpt. */
  excerptOnly: boolean;
  /** The Article was cut; the answer is based on the first `wordsUsed` words. */
  truncated: boolean;
  wordsUsed: number;
  /** Ask only: how many of the oldest turns were left out to stay inside the caps. */
  droppedTurns: number | null;
}

export interface AssistantResult {
  text: string;
  structured: unknown | null;
  /** Set for Article actions only. */
  article?: ArticleNotes | null;
  /** Set for Translate only: the answer already sanitized in the Rust core, images restored. */
  html?: string | null;
}

/**
 * The validated answer to a 'searchQuery' run (#122): the shell already checked it against the
 * Library, so scope names are ids and unknown ones were dropped.
 */
export interface SearchQueryResult {
  /** The FTS5 MATCH expression to run locally. */
  query: string;
  /** Article Date at or after this unix second. */
  since: number | null;
  /** Article Date at or before this unix second. */
  until: number | null;
  feed: number | null;
  folder: number | null;
  /** Scope names that matched nothing in the Library. */
  dropped: string[];
}

/** Narrow a run's structured payload to a SearchQueryResult, or null when it isn't one. */
export function parseSearchQueryResult(v: unknown): SearchQueryResult | null {
  if (typeof v !== 'object' || v === null) return null;
  const o = v as Record<string, unknown>;
  if (typeof o.query !== 'string' || !o.query.trim()) return null;
  const num = (x: unknown) => (typeof x === 'number' && Number.isFinite(x) ? x : null);
  const strs = (x: unknown) => (Array.isArray(x) && x.every((s) => typeof s === 'string') ? (x as string[]) : []);
  return {
    query: o.query,
    since: num(o.since),
    until: num(o.until),
    feed: num(o.feed),
    folder: num(o.folder),
    dropped: strs(o.dropped),
  };
}

export type AssistantErrorKind =
  | 'notInstalled'
  | 'notSignedIn'
  | 'usageLimit'
  | 'offline'
  | 'timedOut'
  | 'cancelled'
  | 'malformed'
  | 'failed'
  /** No machine-local setup, or its disclosure isn't accepted yet: nothing was run. */
  | 'notSetUp'
  /** The picks run needs the user's interests first (#121). */
  | 'noInterests'
  /** The action did not fit the current View (for example no Articles). */
  | 'invalid';

export const assistantNames: Record<AssistantKind, string> = {
  claudeCode: 'Claude Code',
  mistralVibe: 'Mistral Vibe',
  codex: 'Codex',
  gemini: 'Gemini CLI',
  ollama: 'Ollama',
};

export const assistantProviders: Record<AssistantKind, string> = {
  claudeCode: 'Anthropic',
  mistralVibe: 'Mistral',
  codex: 'OpenAI',
  gemini: 'Google',
  ollama: 'Ollama on this computer',
};

/** Nothing leaves the computer: the model runs here. */
export function isLocalAssistant(kind: AssistantKind): boolean {
  return kind === 'ollama';
}

export function detectAssistants(manualPath: string | null = null): Promise<FoundAssistant[]> {
  return invoke('assistant_detect', { manualPath });
}

export function getAssistantSetup(): Promise<AssistantSetup | null> {
  return invoke('assistant_setup_get');
}

export function setAssistantSetup(setup: AssistantSetup | null): Promise<void> {
  return invoke('assistant_setup_set', { setup });
}

export interface RunningTask {
  runId: string;
  result: Promise<AssistantResult>;
  cancel: () => Promise<void>;
}

let nextRun = 0;

/** Start a run. `onEvent` gets the streamed deltas; `result` settles once it's done. */
export function runAssistant(
  action: AssistantAction,
  onEvent: (event: AssistantEvent) => void,
): RunningTask {
  const runId = `run-${Date.now()}-${nextRun++}`;
  const result = (async () => {
    const unlisten = await listen<{ runId: string; event: AssistantEvent }>(
      'assistant-event',
      ({ payload }) => {
        if (payload.runId === runId) onEvent(payload.event);
      },
    );
    try {
      return await invoke<AssistantResult>('assistant_run', { runId, action });
    } finally {
      unlisten();
    }
  })();
  return { runId, result, cancel: () => invoke('assistant_cancel', { runId }) };
}
