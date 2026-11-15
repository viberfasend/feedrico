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

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type AssistantKind = 'claudeCode' | 'mistralVibe';
export type SignIn = 'plan' | 'apiKey' | 'notSignedIn' | 'unknown';

export interface FoundAssistant {
  kind: AssistantKind;
  path: string;
  version: string | null;
  signIn: SignIn;
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
}

/** What to run. Wave 1 has only 'test'; each feature issue adds its action. */
export type AssistantAction =
  | { kind: 'test' }
  | { kind: 'summarize'; articleId: number }
  | { kind: 'explain'; articleId: number }
  | { kind: 'explainTerm'; articleId: number; term: string; context: string }
  | { kind: 'translate'; articleId: number }
  | { kind: 'ask'; articleId: number; question: string; history: { question: string; answer: string }[] };

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
  | 'notSetUp';

export const assistantNames: Record<AssistantKind, string> = {
  claudeCode: 'Claude Code',
  mistralVibe: 'Mistral Vibe',
};

export const assistantProviders: Record<AssistantKind, string> = {
  claudeCode: 'Anthropic',
  mistralVibe: 'Mistral',
};

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
