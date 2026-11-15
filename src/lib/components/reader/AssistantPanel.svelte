<script lang="ts">
  // Where Assistant results appear, above the Article body (#115). Generic: it shows whatever
  // run `assist` holds. It fills the reader's width and never takes focus.
  import { onDestroy, onMount, untrack } from 'svelte';
  import { assistantNames } from '../../assistant';
  import { assist, describeError } from '../../assistant-state.svelte';
  import { app } from '../../state.svelte';
  import Button from '../../ui/Button.svelte';

  const run = $derived(assist.run);
  const name = $derived(assist.setup ? assistantNames[assist.setup.kind] : '');
  const error = $derived(run?.error && assist.setup ? describeError(run.error, assist.setup.kind) : null);

  onMount(() => void assist.loadSetup());

  // Another Article: the run is cancelled and its result thrown away. The setup is re-read so
  // turning Assistants off in Settings takes effect without a restart.
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

  {#if run}
    <section class="panel" aria-label="Assistant: {run.entry.title}" data-phase={run.phase}>
      <header>
        <p class="label">Written by {name} · may be wrong</p>
        <p class="title">{run.entry.title}</p>
      </header>

      <!-- What the input was, so the answer is read honestly: excerpt-only Feeds and cut Articles. -->
      {#if run.article?.excerptOnly}
        <p class="note">This Feed only gives an excerpt; the summary covers that excerpt.</p>
      {/if}
      {#if run.article?.truncated}
        <p class="note">This Article is long; the answer covers the first {run.article.wordsUsed} words.</p>
      {/if}

      {#if error}
        <div class="error">
          <p class="what">{error.what}</p>
          <p class="next">{error.next}</p>
        </div>
      {:else if run.phase === 'running'}
        <p class="stream" aria-busy="true">{run.text || 'Working…'}</p>
      {:else}
        <!-- Escaped and reduced to a few tags by assistant-markdown.ts; never raw output. -->
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
