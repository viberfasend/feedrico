<script lang="ts">
  import { onMount } from 'svelte';
  import { shortDate } from '../format';
  import { app } from '../state.svelte';
  import EmptyState from './EmptyState.svelte';
  import Icon from './Icon.svelte';

  // Virtualized: only rows near the viewport exist in the DOM. Row height comes from the
  // density tokens, so comfortable/compact switches stay exact.
  const OVERSCAN = 10;
  let viewport = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let height = $state(0);
  let rowHeight = $state(76);

  function measure() {
    const value = getComputedStyle(document.documentElement).getPropertyValue(
      '--article-list-row-height',
    );
    rowHeight = parseFloat(value) || 76;
  }

  onMount(() => {
    measure();
    const observer = new MutationObserver(measure);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-density'],
    });
    return () => observer.disconnect();
  });

  const first = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN));
  const last = $derived(Math.min(app.total, Math.ceil((scrollTop + height) / rowHeight) + OVERSCAN));

  $effect(() => {
    void app.ensureRange(first, last);
  });

  const rows = $derived.by(() => {
    void app.pagesVersion;
    const out = [];
    for (let index = first; index < last; index++) out.push({ index, row: app.rowAt(index) });
    return out;
  });

  // A new list starts at the top.
  $effect(() => {
    void app.listEpoch;
    if (viewport) viewport.scrollTop = 0;
  });

  // Keep the selected row in view during keyboard navigation.
  $effect(() => {
    const index = app.selectedIndex;
    if (index === null || !viewport) return;
    const top = index * rowHeight;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + rowHeight > viewport.scrollTop + viewport.clientHeight)
      viewport.scrollTop = top + rowHeight - viewport.clientHeight;
  });

  const title = $derived.by(() => {
    if (app.search.trim()) return `Search: ${app.search.trim()}`;
    const s = app.selection;
    const sidebar = app.sidebar;
    switch (s.kind) {
      case 'allUnread':
        return 'All Unread';
      case 'today':
        return 'Today';
      case 'starred':
        return 'Starred';
      case 'folder':
        return sidebar?.folders.find((f) => f.id === s.id)?.name ?? 'Folder';
      case 'feed':
        return (
          [...(sidebar?.feeds ?? []), ...(sidebar?.folders.flatMap((f) => f.feeds) ?? [])].find(
            (f) => f.id === s.id,
          )?.title ?? 'Feed'
        );
    }
  });

  const hasFeeds = $derived((app.library?.feedCount ?? 0) > 0);
  const refreshing = $derived(app.refresh !== null && !app.refresh.finished);

  let searchTimer: ReturnType<typeof setTimeout>;
  function onSearch(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => app.setSearch(value), 150);
  }
</script>

<section class="list" aria-label="Articles">
  <header class="header">
    <div class="heading">
      <h1>{title}</h1>
      <span class="total">{app.total.toLocaleString()}</span>
    </div>
    <div class="controls">
      <label class="search">
        <Icon name="search" size={15} />
        <input
          id="search"
          type="search"
          placeholder="Search"
          value={app.search}
          oninput={onSearch}
          onkeydown={(e) => {
            if (e.key === 'Escape') {
              app.setSearch('');
              (e.currentTarget as HTMLInputElement).blur();
            }
          }}
        />
      </label>
      <button
        class="tool"
        title={app.sort === 'newestFirst' ? 'Newest first' : 'Oldest first'}
        aria-label={app.sort === 'newestFirst' ? 'Sorted newest first' : 'Sorted oldest first'}
        onclick={() => app.toggleSort()}
      ><Icon name="sort" size={16} /></button>
      <button class="tool" title="Mark all as read" aria-label="Mark all as read" onclick={() => app.markAllRead()}>
        <Icon name="check" size={16} />
      </button>
      <button
        class="tool"
        class:spinning={refreshing}
        title="Refresh (r)"
        aria-label="Refresh"
        onclick={() => app.startRefresh(true)}
      ><Icon name="refresh" size={16} /></button>
    </div>
    {#if app.refresh}
      <div class="progress" role="progressbar" aria-valuemin={0} aria-valuemax={app.refresh.total} aria-valuenow={app.refresh.done}>
        <div class="bar" style:width="{app.refresh.total ? (app.refresh.done / app.refresh.total) * 100 : 100}%"></div>
      </div>
    {/if}
  </header>

  {#if app.hasNewArticles}
    <button class="new-pill" onclick={() => app.resetList()}>New Articles · show</button>
  {/if}

  {#if app.total === 0}
    <div class="empty">
      {#if !hasFeeds}
        <EmptyState title="No Feeds yet">Import an OPML file from the sidebar to get started.</EmptyState>
      {:else if refreshing}
        <EmptyState title="Fetching your Feeds…">Articles will drift in here in a moment.</EmptyState>
      {:else if app.search.trim()}
        <EmptyState title="No matches">Try fewer or shorter words.</EmptyState>
      {:else if app.selection.kind === 'allUnread'}
        <EmptyState title="All caught up">Enjoy the late-summer sun.</EmptyState>
      {:else}
        <EmptyState title="Nothing here">No Articles in this view.</EmptyState>
      {/if}
    </div>
  {:else}
    <div
      class="viewport"
      bind:this={viewport}
      bind:clientHeight={height}
      onscroll={(e) => (scrollTop = (e.currentTarget as HTMLDivElement).scrollTop)}
      role="listbox"
      aria-label={title}
      tabindex="-1"
    >
      <div class="spacer" style:height="{app.total * rowHeight}px">
        {#each rows as { index, row } (index)}
          <div
            class="row"
            class:unread={row && !row.isRead}
            class:selected={app.selectedIndex === index}
            style:transform="translateY({index * rowHeight}px)"
            role="option"
            aria-selected={app.selectedIndex === index}
            tabindex="-1"
            onclick={() => app.selectIndex(index)}
            onkeydown={(e) => e.key === 'Enter' && app.selectIndex(index)}
          >
            {#if row}
              <div class="meta">
                <span class="dot" aria-hidden="true"></span>
                <span class="feed">{row.feedTitle}</span>
                {#if row.isStarred}<span class="star"><Icon name="star" size={12} filled label="Starred" /></span>{/if}
                <time>{shortDate(row.articleDate)}</time>
              </div>
              <div class="title">{row.title}</div>
              <div class="excerpt">{row.excerpt}</div>
            {:else}
              <div class="skeleton"></div>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</section>

<style>
  .list {
    position: relative;
    display: grid;
    grid-template-rows: auto 1fr;
    min-width: 0;
    min-height: 0;
    height: 100%;
    background: var(--color-surface-base);
    border-inline-end: 1px solid var(--color-border-subtle);
  }
  .header {
    position: relative;
    display: grid;
    gap: var(--space-stack-sm);
    padding: var(--space-inset-lg) var(--space-inset-lg) var(--space-inset-md);
    border-block-end: 1px solid var(--color-border-subtle);
  }
  .heading {
    display: flex;
    align-items: baseline;
    gap: var(--space-inline-sm);
    min-width: 0;
  }
  h1 {
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-md);
    font-weight: var(--font-weight-heading);
    line-height: var(--font-line-height-heading);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .total {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
    font-variant-numeric: tabular-nums;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: var(--space-inline-xs);
  }
  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--space-inline-xs);
    height: var(--control-height);
    padding-inline: var(--space-inset-sm);
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    color: var(--color-text-muted);
  }
  .search:focus-within {
    border-color: var(--color-border-focus);
  }
  input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    color: var(--color-text-default);
    font: inherit;
    outline: none;
  }
  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-height);
    height: var(--control-height);
    border: 0;
    border-radius: var(--radius-control);
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .tool:hover {
    background: var(--color-surface-hover);
    color: var(--color-text-default);
  }
  .spinning :global(svg) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }
  .progress {
    position: absolute;
    inset-inline: 0;
    inset-block-end: -1px;
    height: 2px;
  }
  .bar {
    height: 100%;
    background: var(--color-accent-default);
    transition: width var(--motion-duration-base) var(--motion-easing-standard);
  }
  .new-pill {
    position: absolute;
    z-index: 2;
    inset-block-start: calc(var(--space-inset-2xl) * 2.4);
    inset-inline-start: 50%;
    translate: -50% 0;
    padding: var(--space-inset-xs) var(--space-inset-md);
    border: 0;
    border-radius: var(--radius-pill);
    background: var(--color-accent-default);
    color: var(--color-text-on-accent);
    font-size: var(--font-size-small);
    font-weight: var(--font-weight-strong);
    box-shadow: var(--shadow-raised);
    cursor: pointer;
  }
  .empty {
    min-height: 0;
  }
  .viewport {
    overflow-y: auto;
    min-height: 0;
    contain: strict;
    outline: none;
  }
  .spacer {
    position: relative;
  }
  .row {
    position: absolute;
    inset-inline: 0;
    top: 0;
    height: var(--article-list-row-height);
    padding: var(--article-list-row-padding-block) var(--article-list-row-padding-inline);
    border-block-end: 1px solid var(--color-border-subtle);
    display: grid;
    grid-template-rows: auto auto 1fr;
    gap: var(--space-stack-2xs);
    overflow: hidden;
    cursor: pointer;
    contain: layout paint;
  }
  .row:hover {
    background: var(--color-surface-hover);
  }
  .row.selected {
    background: var(--color-surface-selected);
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--space-inline-xs);
    min-width: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .dot {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: var(--radius-pill);
    background: transparent;
  }
  .unread .dot {
    background: var(--color-unread-indicator);
  }
  .feed {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .star {
    display: inline-flex;
    color: var(--color-text-accent);
  }
  time {
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-muted);
    font-weight: var(--font-weight-medium);
  }
  .unread .title {
    color: var(--color-text-default);
    font-weight: var(--font-weight-strong);
  }
  .excerpt {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  :global([data-density='compact']) .excerpt {
    display: none;
  }
  .skeleton {
    grid-row: 1 / -1;
    margin-block: var(--space-stack-xs);
    border-radius: var(--radius-control);
    background: var(--color-surface-hover);
  }
</style>
