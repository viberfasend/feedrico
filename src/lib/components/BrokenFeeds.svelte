<script lang="ts">
  // One place to deal with broken Feeds (#66): each one with its Folder, last error and last
  // success, and actions to open the website, retry now, or unsubscribe. The sidebar keeps its
  // quiet badge; this dialog is opened on purpose. Rows are virtualized: an Archivist may have
  // hundreds of broken Feeds.
  import { SvelteSet } from 'svelte/reactivity';
  import { app } from '../state.svelte';
  import { tokens } from '../tokens/runtime.svelte';
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import FeedIcon from './FeedIcon.svelte';

  const OVERSCAN = 6;
  const rowHeight = $derived(tokens.px('--broken-feeds-row-height') || 56);
  let scrollTop = $state(0);
  let viewHeight = $state(0);

  const items = $derived(app.brokenFeeds);
  const first = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN));
  const last = $derived(Math.min(items.length, Math.ceil((scrollTop + viewHeight) / rowHeight) + OVERSCAN));
  const visible = $derived(items.slice(first, last));

  const retrying = new SvelteSet<number>();
  let status = $state('');
  /** Feeds to unsubscribe from, waiting for confirmation. */
  let confirm = $state<{ ids: number[]; name: string } | null>(null);

  const feedsLabel = (n: number) => (n === 1 ? '1 Feed' : `${n} Feeds`);
  const lastWorked = (at: number | null) =>
    at ? `last worked ${new Date(at * 1000).toLocaleDateString()}` : 'has never worked';

  async function retry(id: number, title: string) {
    retrying.add(id);
    status = `Retrying ${title}…`;
    const error = await app.retryFeed(id);
    retrying.delete(id);
    status = error === null ? `${title} works again` : `${title} still fails: ${error}`;
  }

  async function unsubscribe() {
    const target = confirm;
    confirm = null;
    if (!target) return;
    if (await app.unsubscribeFeeds(target.ids)) status = `Unsubscribed from ${target.name}`;
  }

  // Cancel takes focus when a confirmation appears, so Enter never removes anything.
  let confirmBox = $state<HTMLDivElement>();
  $effect(() => {
    confirmBox?.querySelector<HTMLButtonElement>('button')?.focus();
  });

  // Nothing left to show: close, and say why.
  $effect(() => {
    if (app.brokenFeedsOpen && items.length === 0) {
      app.brokenFeedsOpen = false;
      app.notify('No broken Feeds left');
    }
  });
</script>

<Dialog bind:open={app.brokenFeedsOpen} title="Broken Feeds" class="broken-feeds-dialog">
  <p class="intro">
    {feedsLabel(items.length)} failed on their last fetch. Feedrico keeps retrying them on its own,
    less often each time.
  </p>
  <div
    class="list"
    role="list"
    aria-label="Broken Feeds"
    bind:clientHeight={viewHeight}
    onscroll={(e) => (scrollTop = (e.currentTarget as HTMLDivElement).scrollTop)}
  >
    <div class="spacer" style:height="{items.length * rowHeight}px">
      {#each visible as { feed, folder }, i (feed.id)}
        <div
          class="row"
          role="listitem"
          aria-setsize={items.length}
          aria-posinset={first + i + 1}
          style:height="{rowHeight}px"
          style:transform="translateY({(first + i) * rowHeight}px)"
        >
          <FeedIcon feedId={feed.id} title={feed.title} favicon={feed.favicon} />
          <div class="text">
            <div class="title">{feed.title}</div>
            <div class="meta" title={feed.lastError}>
              {#if folder}<span class="folder">{folder}</span> ·{/if}
              <span class="error">{feed.lastError}</span>
              · <span>{lastWorked(feed.lastSuccessAt)}</span>
            </div>
          </div>
          <div class="row-actions">
            {#if feed.siteUrl}
              <IconButton
                icon="external"
                label={`Open the website of ${feed.title}`}
                title="Open website"
                onclick={() => app.openExternal(feed.siteUrl!)}
              />
            {/if}
            <IconButton
              icon="refresh"
              label={retrying.has(feed.id) ? `Retrying ${feed.title}` : `Retry ${feed.title} now`}
              title="Retry now"
              disabled={retrying.has(feed.id)}
              onclick={() => void retry(feed.id, feed.title)}
            />
            <IconButton
              icon="close"
              label={`Unsubscribe from ${feed.title}`}
              title="Unsubscribe…"
              onclick={() => (confirm = { ids: [feed.id], name: feed.title })}
            />
          </div>
        </div>
      {/each}
    </div>
  </div>
  <p class="sr-only" role="status">{status}</p>

  {#if confirm}
    {@const count = confirm.ids.length}
    <div class="confirm" role="group" aria-label="Confirm unsubscribe" bind:this={confirmBox}>
      <p>
        Unsubscribe from {count === 1 ? confirm.name : feedsLabel(count)}? Their Articles and cached
        images are removed from this Library. This can't be undone.
      </p>
      <div class="actions">
        <Button onclick={() => (confirm = null)}>Cancel</Button>
        <Button variant="primary" onclick={() => void unsubscribe()}>
          Unsubscribe from {count === 1 ? confirm.name : feedsLabel(count)}
        </Button>
      </div>
    </div>
  {:else}
    <div class="actions">
      <Button
        variant="danger"
        onclick={() => (confirm = { ids: items.map((b) => b.feed.id), name: feedsLabel(items.length) })}
      >
        Unsubscribe from all {feedsLabel(items.length)}…
      </Button>
      <Button onclick={() => (app.brokenFeedsOpen = false)}>Close</Button>
    </div>
  {/if}
</Dialog>

<style>
  :global(.broken-feeds-dialog) {
    width: min(var(--dialog-wide-width), calc(100vw - 2rem)) !important;
  }
  .intro {
    margin: 0 0 var(--space-stack-md);
    color: var(--color-text-muted);
  }
  .list {
    position: relative;
    max-height: var(--broken-feeds-list-max-height);
    overflow-y: auto;
    overscroll-behavior-y: none;
    border-block: 1px solid var(--color-border-subtle);
  }
  .spacer {
    position: relative;
  }
  .row {
    position: absolute;
    inset-inline: 0;
    display: flex;
    align-items: center;
    gap: var(--space-inline-sm);
    padding-inline: var(--space-inset-sm);
    border-block-end: 1px solid var(--color-border-subtle);
  }
  .row:focus-within {
    background: var(--color-surface-hover);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title,
  .meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-weight: var(--font-weight-medium);
  }
  .meta {
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .error {
    color: var(--color-status-danger-text);
  }
  .row-actions {
    flex: none;
    display: flex;
    gap: var(--space-inline-xs);
  }
  .confirm p {
    margin: var(--space-stack-md) 0;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-inline-sm);
    margin-block-start: var(--space-stack-md);
  }
</style>
