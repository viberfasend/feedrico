<script lang="ts">
  // Where View-level Assistant results appear, above the Article list (#120 briefing, #121
  // picks). Same rules as the reader's panel (#115): the result names the Assistant and says it
  // may be wrong, a polite live region announces completion, Stop works while running, and every
  // string is escaped by Svelte (never raw Assistant output). Links open the Article exactly
  // like a click in the list.
  import { onDestroy, onMount, untrack } from 'svelte';
  import { api } from '../api';
  import { assistantNames, isBriefingResult, isPicksResult } from '../assistant';
  import { assist, describeError } from '../assistant-state.svelte';
  import { app } from '../state.svelte';
  import Button from '../ui/Button.svelte';

  const run = $derived(assist.viewRun);
  const name = $derived(assist.setup ? assistantNames[assist.setup.kind] : '');
  const error = $derived(run?.error && assist.setup ? describeError(run.error, assist.setup.kind) : null);
  const briefing = $derived(run?.structured && isBriefingResult(run.structured) ? run.structured : null);
  const picks = $derived(run?.structured && isPicksResult(run.structured) ? run.structured : null);
  const articlesById = $derived(new Map((briefing?.articles ?? []).map((a) => [a.id, a])));

  // Which linked Articles are in the current View, so their links can open them like a list click.
  let openable = $state<Set<number>>(new Set());
  $effect(() => {
    if (run?.phase !== 'done' || (!briefing && !picks)) return;
    const ids = briefing
      ? [...new Set(briefing.topics.flatMap((t) => t.articleIds))]
      : [...new Set((picks?.picks ?? []).map((p) => p.id))];
    const query = app.query();
    void Promise.all(ids.map((id) => api.articleIndex(query, id).catch(() => null))).then((indexes) => {
      openable = new Set(ids.filter((_, i) => indexes[i] !== null));
    });
  });

  /** "from the newest 50 Articles" / "from all 12 Articles", matching what the prompt said. */
  function basis(considered: number, capped: boolean): string {
    if (!capped) return `all ${considered} Articles`;
    return app.sort === 'newestFirst' ? `the newest ${considered} Articles` : `the oldest ${considered} Articles`;
  }

  async function open(id: number) {
    const ok = await app.openArticleById(id);
    if (!ok) app.notify('That Article is not in this View');
  }

  // The View changed: the result belonged to the old View. The first run after mount is the
  // run that mounted the panel, so it is not discarded.
  let primed = false;
  $effect(() => {
    void app.selection;
    void app.search;
    if (!primed) {
      primed = true;
      return;
    }
    untrack(() => assist.discardView());
  });
  onDestroy(() => assist.discardView());
  onMount(() => void assist.loadSetup());

  /** Capture phase, so Esc stops a run before anything else sees it. */
  function onEscape(event: KeyboardEvent) {
    if (event.key !== 'Escape' || assist.viewRun?.phase !== 'running') return;
    event.preventDefault();
    event.stopImmediatePropagation();
    assist.stopView();
  }
  onMount(() => {
    window.addEventListener('keydown', onEscape, true);
    return () => window.removeEventListener('keydown', onEscape, true);
  });

  function close() {
    const hadFocus = document.activeElement instanceof HTMLElement && document.activeElement.closest('.vpanel');
    assist.discardView();
    // Focus was on a control that no longer exists: hand it back to the list, not to the page.
    if (hadFocus) document.getElementById('pane-list')?.focus({ preventScroll: true });
  }
</script>

{#if assist.setup?.disclosureAccepted}
  <!-- Announces completion once; the streamed text itself is not in a live region. -->
  <div class="sr-only" role="status" aria-live="polite">{assist.announcement}</div>

  {#if run}
    <section class="vpanel" aria-label="Assistant: {run.entry.title}" data-phase={run.phase}>
      <header>
        <p class="label">Written by {name} · may be wrong</p>
        <p class="title">{run.entry.title}</p>
      </header>

      {#if error}
        <div class="error">
          <p class="what">{error.what}</p>
          <p class="next">{error.next}</p>
        </div>
      {:else if run.phase === 'running'}
        <p class="stream" aria-busy="true">Working…</p>
      {:else if briefing}
        <p class="meta">Based on {basis(briefing.considered, briefing.capped)}</p>
        {#if briefing.topics.length}
          <ul class="topics">
            {#each briefing.topics as t (t.topic + t.articleIds.join(','))}
              <li>
                <strong>{t.topic}.</strong>
                {t.sentence}
                <span class="links">
                  {#each t.articleIds as id (id)}
                    {@const a = articlesById.get(id)}
                    {#if a}
                      {#if openable.has(id)}
                        <button type="button" class="alink" title={`Open in the reader (from ${a.feedTitle})`} onclick={() => void open(id)}>{a.title}</button>
                      {:else}
                        <span class="alink dead" title={`${a.feedTitle} — not in this View`}>{a.title}</span>
                      {/if}
                    {/if}
                  {/each}
                </span>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="meta">No topics came back.</p>
        {/if}
      {:else if picks}
        <p class="meta">Ranked {basis(picks.considered, picks.capped)} of this View (unread)</p>
        {#if picks.picks.length}
          <ol class="picks">
            {#each picks.picks as p (p.id)}
              <li>
                {#if openable.has(p.id)}
                  <button type="button" class="alink" title={`Open in the reader (from ${p.feedTitle})`} onclick={() => void open(p.id)}>{p.title}</button>
                {:else}
                  <span class="alink dead" title={`${p.feedTitle} — not in this View`}>{p.title}</span>
                {/if}
                <span class="reason">{p.reason}</span>
                <span class="score" title="score">{Math.round(p.score * 100)}</span>
              </li>
            {/each}
          </ol>
        {:else}
          <p class="meta">No picks came back.</p>
        {/if}
      {:else}
        <p class="meta">No readable answer came back.</p>
      {/if}

      <div class="actions">
        {#if run.phase === 'running'}
          <Button onclick={() => assist.stopView()}>Stop <kbd>Esc</kbd></Button>
        {:else}
          <Button variant="ghost" onclick={() => assist.start(run.entry)}><span data-action="retry">Try again</span></Button>
        {/if}
        <Button variant="ghost" onclick={close}><span data-action="close">Close</span></Button>
      </div>
    </section>
  {/if}
{/if}

<style>
  .vpanel {
    margin: var(--space-inset-md) var(--space-inset-lg) 0;
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
    /* A long picks list scrolls inside the panel instead of squeezing the Article list away;
       the list's grid caps the panel's row (ArticleList.svelte). */
    min-block-size: 0;
    overflow-y: auto;
  }
  @media (prefers-reduced-motion: no-preference) {
    .vpanel {
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
  .meta {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .title {
    font-weight: var(--font-weight-medium);
  }
  .stream {
    white-space: pre-wrap;
  }
  .topics,
  .picks {
    margin: 0;
    padding-inline-start: 1.4em;
    display: grid;
    gap: var(--space-stack-xs);
  }
  .picks {
    list-style: none;
    padding-inline-start: 0;
  }
  .links {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--space-inline-xs);
    margin-inline-start: var(--space-inline-xs);
  }
  .alink {
    padding: 0 var(--space-inset-xs);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-pill);
    background: none;
    color: var(--color-text-link);
    font: inherit;
    font-size: var(--font-size-small);
    text-underline-offset: 0.18em;
    cursor: pointer;
  }
  .alink.dead {
    color: var(--color-text-muted);
    cursor: default;
  }
  .reason {
    display: block;
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .score {
    margin-inline-start: var(--space-inline-xs);
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
    font-variant-numeric: tabular-nums;
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
