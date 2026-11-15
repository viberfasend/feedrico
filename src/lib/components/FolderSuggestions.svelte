<script lang="ts">
  // The preview for Assistant Folder suggestions (#123), on the Feed management surface: a list
  // of proposed moves with per-Feed accept/reject and accept/reject all, progress while the
  // batches run, Stop, and Apply as one undoable step (the toast's Undo). Keyboard operable:
  // every control is a native checkbox or button; the Dialog returns focus on close.
  import { assist, describeError } from '../assistant-state.svelte';
  import { BATCH_SIZE, folderSuggestions } from '../folder-suggestions.svelte';
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Icon from './Icon.svelte';

  const s = folderSuggestions;
  const error = $derived(s.error && assist.setup ? describeError(s.error, assist.setup.kind) : null);
  const title = $derived(
    s.scope?.kind === 'folder' ? `Suggest Folders for ${s.scope.name}` : 'Suggest Folders for unsorted Feeds',
  );
  const progressEnd = $derived(Math.min(s.doneFeeds + BATCH_SIZE, s.totalFeeds));
</script>

{#if s.open}
  <Dialog
    open={true}
    onOpenChange={(o) => {
      if (!o) s.close();
    }}
    title={title}
    class="folder-suggestions-dialog"
  >
    <!-- Announces progress and completion once; never streamed tokens. -->
    <div class="sr-only" role="status" aria-live="polite">{s.announcement}</div>
    <p class="label">Proposed by {s.assistantName} · may be wrong</p>

    {#if s.phase === 'running'}
      <p class="progress" aria-busy="true">
        Asking about {s.doneFeeds + 1}–{progressEnd} of {s.totalFeeds} Feeds · group {s.batch} of {s.batches}
      </p>
    {:else if error}
      <div class="error">
        <p class="what">{error.what}</p>
        <p class="next">{error.next}</p>
      </div>
    {/if}

    {#if s.proposals.length}
      <div class="toolbar">
        <Button variant="ghost" onclick={() => s.setAll(true)}>Accept all</Button>
        <Button variant="ghost" onclick={() => s.setAll(false)}>Reject all</Button>
        <span class="count">{s.acceptedCount} of {s.proposals.length} accepted</span>
      </div>
      <ul class="moves">
        {#each s.proposals as p (p.feedId)}
          <li>
            <label class="move">
              <input
                type="checkbox"
                checked={p.accepted}
                onchange={() => s.toggle(p.feedId)}
                aria-label={`Move ${p.feedTitle} to ${p.folder}`}
              />
              <span class="feed" title={p.feedTitle}>{p.feedTitle}</span>
              <Icon name="chevron" size={12} />
              <span class="folder">
                {p.folder}
                {#if p.isNewFolder}<span class="new">new Folder</span>{/if}
              </span>
            </label>
          </li>
        {/each}
      </ul>
    {:else if s.phase === 'done' || s.phase === 'stopped'}
      <p class="empty">No suggestions — the Feeds stay where they are.</p>
    {/if}

    <div class="actions">
      {#if s.phase === 'running'}
        <Button onclick={() => s.stop()}>Stop</Button>
      {:else if error}
        <Button
          onclick={() => {
            const scope = s.scope;
            if (scope) s.start(scope);
          }}>Try again</Button
        >
      {/if}
      <Button onclick={() => s.close()}>{s.phase === 'running' ? 'Cancel' : 'Close'}</Button>
      {#if s.phase !== 'running' && !error}
        <Button
          variant="primary"
          disabled={s.acceptedCount === 0 || s.applying}
          onclick={() => void s.apply()}
        >
          {s.applying ? 'Moving…' : `Move ${s.acceptedCount} ${s.acceptedCount === 1 ? 'Feed' : 'Feeds'}`}
        </Button>
      {/if}
    </div>
  </Dialog>
{/if}

<style>
  .label {
    margin: 0 0 var(--space-stack-sm);
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .progress {
    margin: 0 0 var(--space-stack-sm);
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .error .what {
    margin: 0;
    font-weight: var(--font-weight-medium);
  }
  .error .next {
    margin: var(--space-stack-xs, 0.25em) 0 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-inline-sm);
    margin-block-end: var(--space-stack-sm);
  }
  .count {
    margin-inline-start: auto;
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .moves {
    list-style: none;
    margin: 0 0 var(--space-stack-md);
    padding: 0;
    display: grid;
    gap: 1px;
    max-height: 50vh;
    overflow-y: auto;
    overscroll-behavior-y: none;
  }
  .move {
    display: flex;
    align-items: center;
    gap: var(--space-inline-sm);
    min-height: var(--menu-item-height, 32px);
    padding-inline: var(--space-inset-xs);
    border-radius: var(--radius-control);
    cursor: pointer;
  }
  .move:hover {
    background: var(--color-surface-hover);
  }
  .feed {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .folder {
    display: inline-flex;
    align-items: center;
    gap: var(--space-inline-xs);
    color: var(--color-text-muted);
    white-space: nowrap;
  }
  .new {
    padding: 0 var(--space-inset-xs);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-pill);
    font-size: var(--font-size-caption);
  }
  .empty {
    margin: 0 0 var(--space-stack-md);
    color: var(--color-text-muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-inline-sm);
  }
</style>
