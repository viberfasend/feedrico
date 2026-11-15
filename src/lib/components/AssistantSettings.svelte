<script lang="ts">
  // Settings > Assistant: the only place Assistants are turned on. The setup is machine-local
  // (assistant.json in the app data dir), never part of the Library. Nothing runs here until the
  // user presses Test, and Feedrico never opens a login: it only says how to sign in yourself.
  import { onMount } from 'svelte';
  import { isCommandError } from '../api';
  import { assist } from '../assistant-state.svelte';
  import {
    type AssistantKind,
    type AssistantSetup,
    type FoundAssistant,
    assistantNames,
    assistantProviders,
    detectAssistants,
    getAssistantSetup,
    runAssistant,
    setAssistantSetup,
  } from '../assistant';
  import Button from '../ui/Button.svelte';
  import Input from '../ui/Input.svelte';
  import Select from '../ui/Select.svelte';

  const CLAUDE_MODELS = [
    { value: '', label: 'Default' },
    { value: 'haiku', label: 'Haiku (fast)' },
    { value: 'sonnet', label: 'Sonnet' },
    { value: 'opus', label: 'Opus' },
  ];

  let loaded = $state(false);
  let setup = $state<AssistantSetup | null>(null);
  let phase = $state<'closed' | 'detecting' | 'listed'>('closed');
  let found = $state<FoundAssistant[]>([]);
  let manualPath = $state('');
  let problem = $state<string | null>(null);
  let test = $state<{ state: 'idle' | 'running' | 'done' | 'error'; text: string; ms: number; error: string }>({
    state: 'idle',
    text: '',
    ms: 0,
    error: '',
  });

  const ids = $props.id();

  onMount(async () => {
    try {
      setup = await getAssistantSetup();
    } catch {
      setup = null;
    }
    loaded = true;
  });

  async function save(next: AssistantSetup | null) {
    try {
      await setAssistantSetup(next);
      setup = next;
      problem = null;
      // The reader's Assistant entry points follow the setup at once (on, off, accepted).
      void assist.loadSetup();
    } catch {
      problem = 'Could not save this on your computer. Try again.';
    }
  }

  async function look() {
    phase = 'detecting';
    problem = null;
    try {
      found = await detectAssistants(manualPath.trim() || null);
    } catch {
      found = [];
      problem = 'Looking for Assistants did not work. Try again.';
    }
    phase = 'listed';
  }

  function pick(item: FoundAssistant) {
    if (setup && setup.kind === item.kind && setup.path === item.path) return;
    test = { state: 'idle', text: '', ms: 0, error: '' };
    void save({
      kind: item.kind,
      path: item.path,
      model: item.kind === 'claudeCode' ? 'haiku' : null,
      disclosureAccepted: setup?.disclosureAccepted ?? false,
      // Translate's target language defaults to the system language.
      targetLanguage: setup?.targetLanguage ?? navigator.language,
    });
  }

  const setModel = (model: string) => setup && save({ ...setup, model: model.trim() || null });
  const setLanguage = (language: string) => setup && save({ ...setup, targetLanguage: language.trim() || null });
  const accept = (accepted: boolean) => setup && save({ ...setup, disclosureAccepted: accepted });

  async function turnOff() {
    await save(null);
    phase = 'closed';
    found = [];
    manualPath = '';
    test = { state: 'idle', text: '', ms: 0, error: '' };
  }

  function version(item: FoundAssistant) {
    return item.version ? ` ${item.version}` : '';
  }

  const loginHint: Record<AssistantKind, { before: string; command: string; after: string }> = {
    claudeCode: { before: 'not signed in: open a terminal and run ', command: 'claude', after: ' to sign in' },
    mistralVibe: { before: 'not signed in: run ', command: 'vibe --setup', after: '' },
  };

  function errorText(e: unknown): string {
    const kind = isCommandError(e) ? e.kind : 'failed';
    const name = setup ? assistantNames[setup.kind] : 'The Assistant';
    switch (kind) {
      case 'notInstalled':
        return `${name} could not be started. Check the path, or press Look again.`;
      case 'notSignedIn':
        return `${name} is not signed in. Sign in yourself in a terminal, then test again.`;
      case 'usageLimit':
        return `${name} has reached its usage limit. Try again later.`;
      case 'offline':
        return `${name} could not reach its provider. Check your connection.`;
      case 'timedOut':
        return `${name} took too long to answer. Try again.`;
      case 'cancelled':
        return 'Stopped.';
      case 'malformed':
        return `${name} answered in a way Feedrico could not read.`;
      default:
        return `${name} did not work. Try again.`;
    }
  }

  let task: ReturnType<typeof runAssistant> | null = null;

  async function runTest() {
    if (!setup?.disclosureAccepted || test.state === 'running') return;
    const started = performance.now();
    test = { state: 'running', text: '', ms: 0, error: '' };
    try {
      task = runAssistant({ kind: 'test' }, (event) => {
        if (event.kind === 'text') test.text += event.delta;
        else test.text = event.text;
      });
      const result = await task.result;
      test = { state: 'done', text: result.text, ms: Math.round(performance.now() - started), error: '' };
    } catch (e) {
      test = { state: 'error', text: '', ms: 0, error: errorText(e) };
    }
    task = null;
  }

  /** The detected Assistants, plus the one set up if it was not found this time. */
  const listed = $derived.by(() => {
    const items = found.map((item) => ({ item, missing: false }));
    if (setup && !found.some((f) => f.kind === setup!.kind && f.path === setup!.path)) {
      items.unshift({
        item: { kind: setup.kind, path: setup.path, version: null, signIn: 'unknown' },
        missing: true,
      });
    }
    return items;
  });

  const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
  const modelValue = $derived(setup?.model ?? '');
  const languageValue = $derived(setup?.targetLanguage ?? '');
  const systemLanguage = navigator.language;
  const providerLine = $derived(
    setup
      ? `When you ask, Feedrico gives the Article's text to ${assistantNames[setup.kind]} on this computer, which sends it to ${assistantProviders[setup.kind]}${setup.kind === 'claudeCode' ? ' under your Claude plan' : ''}. Nothing is sent unless you ask.`
      : '',
  );
</script>

{#snippet signIn(item: FoundAssistant)}
  {#if item.signIn === 'plan'}
    {item.kind === 'claudeCode' ? 'signed in with your Claude plan' : 'signed in'}
  {:else if item.signIn === 'apiKey'}
    signed in with an API key: runs are billed per use
  {:else if item.signIn === 'notSignedIn'}
    {@const hint = loginHint[item.kind]}
    {hint.before}<code>{hint.command}</code>{hint.after}
  {:else}
    sign-in state unknown
  {/if}
{/snippet}

{#if loaded}
  <div class="assistant">
    <div class="group">
      <div class="row">
        <div class="row-text">
          <span class="label">Assistant</span>
          <span class="desc">
            {#if setup}
              {assistantNames[setup.kind]} on this computer. It only works when you ask.
            {:else}
              An Assistant is an AI tool you already have on this computer, like Claude Code, that
              Feedrico can ask to help with Articles. It is off until you set it up.
            {/if}
          </span>
        </div>
        <div class="actions">
          {#if setup}
            <Button onclick={look} disabled={phase === 'detecting'}>Change</Button>
            <Button variant="danger" onclick={turnOff}>Turn off</Button>
          {:else}
            <Button variant="primary" onclick={look} disabled={phase === 'detecting'}>Set up</Button>
          {/if}
        </div>
      </div>

      {#if phase !== 'closed'}
        <div class="row stack">
          <div class="row-text" role="status" aria-live="polite">
            {#if phase === 'detecting'}
              <span class="desc">Looking for Assistants…</span>
            {:else if listed.length === 0}
              <span class="desc">
                No Assistant found. Install Claude Code or Mistral Vibe, or enter where it is below.
              </span>
            {/if}
          </div>
          {#if phase === 'listed' && listed.length > 0}
            <div class="found" role="radiogroup" aria-label="Assistant to use">
              {#each listed as { item, missing } (item.kind + item.path)}
                <label class="found-item">
                  <input
                    type="radio"
                    name="{ids}-assistant"
                    checked={setup?.kind === item.kind && setup?.path === item.path}
                    onchange={() => pick(item)}
                  />
                  <span class="row-text">
                    <span class="label">{assistantNames[item.kind]}{version(item)}</span>
                    <span class="desc">
                      {#if missing}
                        Not found at {item.path} — check the path or Look again
                      {:else}
                        {@render signIn(item)}
                      {/if}
                    </span>
                    <span class="desc path" title={item.path}>{item.path}</span>
                  </span>
                </label>
              {/each}
            </div>
          {/if}
          <div class="manual">
            <label class="label" for="{ids}-path">Path to the Assistant, if it was not found</label>
            <div class="manual-row">
              <Input id="{ids}-path" bind:value={manualPath} placeholder="/home/you/.local/bin/claude" />
              <Button onclick={look} disabled={phase === 'detecting'}>Look again</Button>
            </div>
          </div>
        </div>
      {/if}

      {#if problem}
        <div class="row"><span class="desc problem" role="alert">{problem}</span></div>
      {/if}

      {#if setup}
        <div class="row">
          <div class="row-text">
            <label class="label" for="{ids}-model">Model</label>
            <span class="desc">
              {setup.kind === 'claudeCode'
                ? 'Haiku is fast and enough for most things.'
                : 'Leave empty to use the model Vibe is set up with.'}
            </span>
          </div>
          <div class="control">
            {#if setup.kind === 'claudeCode'}
              <Select label="Model" value={modelValue} options={CLAUDE_MODELS} onchange={setModel} />
            {:else}
              <Input
                id="{ids}-model"
                value={modelValue}
                placeholder="Default"
                onchange={(e: Event) => setModel((e.currentTarget as HTMLInputElement).value)}
              />
            {/if}
          </div>
        </div>

        <div class="row">
          <div class="row-text">
            <label class="label" for="{ids}-language">Translation language</label>
            <span class="desc">
              Translate renders Articles into this language, for example de or pt-BR. Empty means your
              system language ({systemLanguage}).
            </span>
          </div>
          <div class="control">
            <Input
              id="{ids}-language"
              value={languageValue}
              placeholder={systemLanguage}
              onchange={(e: Event) => setLanguage((e.currentTarget as HTMLInputElement).value)}
            />
          </div>
        </div>

        <div class="row stack">
          <p class="disclosure" id="{ids}-disclosure">{providerLine}</p>
          <label class="accept">
            <input
              type="checkbox"
              checked={setup.disclosureAccepted}
              onchange={(e) => accept(e.currentTarget.checked)}
            />
            <span>I understand, and I want to use this Assistant</span>
          </label>
        </div>

        <div class="row">
          <div class="row-text">
            <span class="label">Test</span>
            <span class="desc" id="{ids}-test-hint">
              {setup.disclosureAccepted
                ? 'Asks the Assistant a tiny question to check that it works.'
                : 'Accept the note above to test.'}
            </span>
          </div>
          <div class="actions">
            <Button disabled={!setup.disclosureAccepted || test.state === 'running'} onclick={runTest}>
              {test.state === 'running' ? 'Testing…' : 'Test'}
            </Button>
          </div>
        </div>
        <div class="row">
          <div class="result" role="status" aria-live="polite" aria-label="Test result">
            {#if test.state === 'running'}
              <span class="desc">Testing…{test.text ? ' ' : ''}{test.text}</span>
            {:else if test.state === 'done'}
              <span class="answer">{test.text}</span>
              <span class="desc">Answered in {seconds(test.ms)}.</span>
            {:else if test.state === 'error'}
              <span class="problem">{test.error}</span>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .group {
    margin: 0;
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-card);
    background: var(--color-surface-base);
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--settings-control-width);
    align-items: center;
    gap: var(--space-inline-lg);
    min-height: 56px;
    padding: var(--space-inset-md) var(--space-inset-lg);
  }
  .row + .row {
    border-block-start: 1px solid var(--color-border-subtle);
  }
  .row.stack {
    grid-template-columns: minmax(0, 1fr);
  }
  .row-text {
    display: grid;
    gap: var(--space-stack-2xs);
    min-width: 0;
  }
  .label {
    font-weight: var(--font-weight-medium);
    color: var(--color-text-default);
  }
  .desc {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .path {
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-inline-md);
  }
  .control {
    min-width: 0;
  }
  .found {
    display: grid;
    gap: var(--space-stack-sm);
  }
  .found-item,
  .accept {
    display: flex;
    gap: var(--space-inline-md);
    align-items: flex-start;
    cursor: pointer;
  }
  .found-item input,
  .accept input {
    margin-block-start: 0.25em;
  }
  .manual {
    display: grid;
    gap: var(--space-stack-xs);
  }
  .manual-row {
    display: flex;
    gap: var(--space-inline-md);
  }
  .manual-row :global(.input) {
    flex: 1;
    min-width: 0;
  }
  .disclosure {
    margin: 0;
    color: var(--color-text-default);
  }
  .result {
    grid-column: 1 / -1;
    display: grid;
    gap: var(--space-stack-xs);
    min-height: 1.5em;
  }
  .answer {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--color-text-default);
  }
  .problem {
    color: var(--color-text-default);
    font-size: var(--font-size-small);
  }
  code {
    font-family: var(--font-family-mono, monospace);
  }
</style>
