<script lang="ts">
  // Where Assistant results appear, above the Article body (#115). Generic: it shows whatever
  // run `assist` holds, plus the Ask question field and conversation (#119). It fills the
  // reader's width and never takes focus.
  import { onDestroy, onMount, untrack } from 'svelte';
  import { assistantNames } from '../../assistant';
  import { assist, describeError } from '../../assistant-state.svelte';
  import { app } from '../../state.svelte';
  import Button from '../../ui/Button.svelte';
  import Input from '../../ui/Input.svelte';

  const run = $derived(assist.run);
  const name = $derived(assist.setup ? assistantNames[assist.setup.kind] : '');
  const error = $derived(run?.error && assist.setup ? describeError(run.error, assist.setup.kind) : null);
  const title = $derived(run?.entry.title ?? 'Ask about this Article');

  onMount(() => void assist.loadSetup());

  // Another Article: the run is cancelled and its result and conversation thrown away. The
  // setup is re-read so turning Assistants off in Settings takes effect without a restart.
  $effect(() => {
    void app.article?.id;
    untrack(() => {
      assist.discard();
      void assist.loadSetup();
    });
  });
  onDestroy(() => assist.discard());

  /** Capture phase, so Esc stops a run before anything else (closing the slide-over) sees it. */
  function onEscape(event: KeyboardEvent) {
    if (event.key !== 'Escape' || assist.run?.phase !== 'running') return;
    event.preventDefault();
    event.stopImmediatePropagation();
    assist.stop();
  }
  onMount(() => {
    window.addEventListener('keydown', onEscape, true);
    return () => window.removeEventListener('keydown', onEscape, true);
  });

  /** The reader's shortcuts: `i` toggles the Assistant menu, `u` summarizes the Article (#116).
   *  Never while typing, with modifiers, or with a menu open. */
  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'i' && event.key !== 'u') return;
    if (event.metaKey || event.ctrlKey || event.altKey || event.defaultPrevented) return;
    if (app.phase !== 'ready' || !assist.available) return;
    const target = event.target;
    if (target instanceof HTMLElement && target.closest('input, textarea, select, [contenteditable="true"]')) return;
    if (event.key === 'u' && target instanceof HTMLElement && target.closest('[role="menu"]')) return;
    event.preventDefault();
    if (event.key === 'i') assist.menuOpen = !assist.menuOpen;
    else assist.startEntry('summarize');
  }

  // Choosing Ask means the user wants to type: move focus to the question field once, a frame
  // later, so it lands after the menu has handed focus back to its trigger.
  $effect(() => {
    if (!assist.askOpen) return;
    const frame = requestAnimationFrame(() => document.getElementById('ask-question')?.focus());
    return () => cancelAnimationFrame(frame);
  });

  /** Enter in the question field sends the question. */
  function onAskKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    assist.ask();
  }

  function onAskSubmit(event: SubmitEvent) {
    event.preventDefault();
    assist.ask();
  }

  function close() {
    const hadFocus = document.activeElement instanceof HTMLElement && document.activeElement.closest('.panel');
    assist.discard();
    // Focus was on a button that no longer exists: hand it back to the reader, not to the page.
    if (hadFocus) document.getElementById('pane-reader')?.focus({ preventScroll: true });
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if assist.setup?.disclosureAccepted}
  <!-- Announces completion once; the streamed text itself is not in a live region. -->
  <div class="sr-only" role="status" aria-live="polite">{assist.announcement}</div>

  {#if run || assist.askOpen}
    <section class="panel" aria-label="Assistant: {title}" data-phase={run?.phase ?? 'ask'}>
      <header>
        <p class="label">Written by {name} · may be wrong</p>
        <p class="title">{title}</p>
      </header>

      {#if assist.askOpen}
        <form class="ask" onsubmit={onAskSubmit}>
          <label class="label" for="ask-question">Ask about this Article</label>
          <div class="ask-row">
            <Input
              id="ask-question"
              bind:value={assist.question}
              placeholder="What does it say about…?"
              disabled={run?.phase === 'running'}
              onkeydown={onAskKeydown}
            />
            <Button type="submit" disabled={!assist.question.trim() || run?.phase === 'running'}>Ask</Button>
          </div>
        </form>
        {#each assist.turns as turn (turn.question)}
          <div class="turn">
            <p class="q">{turn.question}</p>
            <!-- Sanitized like every Assistant output: the core's HTML or renderMarkdown. -->
            <div class="rendered">{@html turn.html}</div>
          </div>
        {/each}
        {#if assist.droppedTurns > 0}
          <p class="note">
            The {assist.droppedTurns} oldest question{assist.droppedTurns === 1 ? '' : 's'}
            {assist.droppedTurns === 1 ? 'was' : 'were'} left out to keep the request small.
          </p>
        {/if}
      {/if}

      {#if run}
        <!-- What the input was, so the answer is read honestly: excerpt-only Feeds and cut Articles. -->
        {#if run.notes?.excerptOnly}
          <p class="note">This Feed only gives an excerpt; the {run.entry.id === 'summarize' ? 'summary' : 'answer'} covers that excerpt.</p>
        {/if}
        {#if run.notes?.truncated}
          <p class="note">This Article is long; the answer covers the first {run.notes.wordsUsed} words.</p>
        {/if}

        {#if error}
          <div class="error">
            <p class="what">{error.what}</p>
            <p class="next">{error.next}</p>
          </div>
        {:else if run.phase === 'running'}
          <p class="stream" aria-busy="true">{run.text || 'Working…'}</p>
        {:else if !(assist.askOpen && run.phase === 'done')}
          <!-- Sanitized in the core (Translate) or escaped and reduced by assistant-markdown.ts.
               A finished Ask answer is already in the conversation above, so it isn't repeated. -->
          <div class="rendered">{@html run.html}</div>
          {#if run.phase === 'stopped'}<p class="note">Stopped. This is what arrived so far.</p>{/if}
        {/if}

        <div class="actions">
          {#if run.phase === 'running'}
            <Button onclick={() => assist.stop()}>Stop <kbd>Esc</kbd></Button>
          {:else}
            {#if run.text && !error}
              <Button variant="ghost" onclick={() => void assist.copy()}><span data-action="copy">Copy</span></Button>
            {/if}
            <Button variant="ghost" onclick={() => assist.retry()}><span data-action="retry">Try again</span></Button>
          {/if}
          <Button variant="ghost" onclick={close}><span data-action="close">Close</span></Button>
        </div>
      {:else}
        <div class="actions">
          <Button variant="ghost" onclick={close}><span data-action="close">Close</span></Button>
        </div>
      {/if}
    </section>
  {/if}
{/if}

<style>
  .panel {
    /* Fills the reader, whatever its width. */
    margin-block: 0 var(--space-stack-md);
    padding: var(--space-inset-md);
    border: 1px solid var(--color-border-subtle);
    border-inline-start: 3px solid var(--color-border-default);
    border-radius: var(--radius-card);
    background: var(--color-surface-sunken);
    color: var(--color-text-default);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-body);
    line-height: var(--font-line-height-body);
    overflow-wrap: anywhere;
  }
  @media (prefers-reduced-motion: no-preference) {
    .panel {
      animation: panel-in var(--motion-duration-base) var(--motion-easing-standard);
    }
  }
  @keyframes panel-in {
    from {
      opacity: 0;
    }
  }
  header {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--space-inline-sm);
    margin-block-end: var(--space-stack-sm);
  }
  p {
    margin: 0;
  }
  .label,
  .title,
  .note {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .title {
    font-weight: var(--font-weight-medium);
  }
  .stream {
    white-space: pre-wrap;
  }
  .ask {
    display: grid;
    gap: var(--space-stack-xs);
    margin-block-end: var(--space-stack-sm);
  }
  .ask .label {
    font-weight: var(--font-weight-medium);
  }
  .ask-row {
    display: flex;
    gap: var(--space-inline-sm);
  }
  .ask-row :global(.input) {
    flex: 1;
    min-width: 0;
  }
  .turn {
    margin-block-end: var(--space-stack-sm);
  }
  .q {
    font-weight: var(--font-weight-medium);
  }
  .turn .rendered {
    margin-block-start: var(--space-stack-2xs, 0.25em);
  }
  .rendered :global(:where(p, ul, ol)) {
    margin-block: 0 0.8em;
  }
  .rendered :global(:where(p, ul, ol):last-child) {
    margin-block-end: 0;
  }
  .rendered :global(:where(ul, ol)) {
    padding-inline-start: 1.4em;
  }
  .rendered :global(a) {
    color: var(--color-text-link);
    text-underline-offset: 0.18em;
    cursor: pointer;
  }
  .note {
    margin-block-start: var(--space-stack-sm);
  }
  .what {
    font-weight: var(--font-weight-medium);
  }
  .next {
    margin-block-start: var(--space-stack-xs, 0.25em);
    color: var(--color-text-muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-inline-sm);
    margin-block-start: var(--space-stack-sm);
  }
  kbd {
    margin-inline-start: var(--space-inline-xs);
    padding: 0 var(--space-inset-xs);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-caption);
  }
</style>
