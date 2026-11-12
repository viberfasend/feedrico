<script lang="ts">
  import { LAYOUTS, type Layout } from '../api';
  import { app } from '../state.svelte';
  import Button from '../ui/Button.svelte';
  import EmptyState from './EmptyState.svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import CardsLayout from './layouts/CardsLayout.svelte';
  import GalleryLayout from './layouts/GalleryLayout.svelte';
  import ListLayout from './layouts/ListLayout.svelte';
  import MagazineLayout from './layouts/MagazineLayout.svelte';

  const layoutMeta: Record<Layout, { label: string; icon: IconName }> = {
    list: { label: 'List', icon: 'list' },
    cards: { label: 'Cards', icon: 'cards' },
    magazine: { label: 'Magazine', icon: 'magazine' },
    gallery: { label: 'Gallery', icon: 'gallery' },
  };

  const viewName = $derived.by(() => {
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
  const title = $derived(app.search.trim() ? `Search: ${app.search.trim()}` : viewName);
  let searchFocused = $state(false);
  const showScope = $derived(app.searchScope !== null && (searchFocused || app.search.trim() !== ''));

  const hasFeeds = $derived((app.library?.feedCount ?? 0) > 0);
  const refreshing = $derived(app.refresh !== null && !app.refresh.finished);

  let searchTimer: ReturnType<typeof setTimeout>;
  function onSearch(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => app.setSearch(value), 150);
  }
</script>

<section class="list" id="pane-list" aria-label="Articles" tabindex="-1">
  <header class="header">
    <div class="heading">
      <h1>{title}</h1>
      {#if !app.hideCounts}<span class="total">{app.total.toLocaleString()}</span>{/if}
    </div>
    <div class="controls">
      <div class="switcher" role="group" aria-label="Layout (v)">
        {#each LAYOUTS as layout (layout)}
          <button
            class="segment"
            aria-pressed={app.layout === layout}
            title={`${layoutMeta[layout].label} (v)`}
            aria-label={layoutMeta[layout].label}
            onclick={() => app.setLayout(layout)}
          ><Icon name={layoutMeta[layout].icon} size={16} /></button>
        {/each}
      </div>
      <label class="search">
        <Icon name="search" size={15} />
        {#if showScope}
          <button
            class="scope"
            title="Search everywhere instead"
            aria-label={`Searching in ${viewName}. Search everywhere instead`}
            onmousedown={(e) => e.preventDefault()}
            onclick={() => void app.searchEverywhere()}
          ><span class="scope-name">in {viewName}</span><Icon name="close" size={12} /></button>
        {/if}
        <input
          id="search"
          type="search"
          placeholder="Search"
          value={app.search}
          oninput={onSearch}
          onfocus={() => (searchFocused = true)}
          onblur={() => (searchFocused = false)}
          onkeydown={(e) => {
            if (e.key === 'Backspace' && e.currentTarget.value === '' && app.searchScope) {
              void app.searchEverywhere();
            }
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
    {#if app.hiddenWithoutImage > 0}
      <p class="note" role="status">
        {app.hiddenWithoutImage.toLocaleString()} Article{app.hiddenWithoutImage === 1 ? '' : 's'} without images not shown
      </p>
    {/if}
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
        <EmptyState title="Add your first Feed">
          Paste the address of a website you like to read. Coming from another reader? Import
          its list of Feeds (an OPML file).
          {#snippet actions()}
            <Button variant="primary" onclick={() => (app.addFeedOpen = true)}>Add a website or Feed…</Button>
            <Button onclick={() => void app.chooseImportOpml()}>Import a list of Feeds…</Button>
          {/snippet}
        </EmptyState>
      {:else if refreshing}
        <EmptyState title="Fetching your Feeds…">Articles will drift in here in a moment.</EmptyState>
      {:else if app.layout === 'gallery' && app.hiddenWithoutImage > 0}
        <EmptyState title="No images here">This View has no Articles with images. Try Cards or List.</EmptyState>
      {:else if app.search.trim()}
        {#if app.searchScope}
          <EmptyState title={`No matches in ${viewName}`}>
            Try fewer or shorter words.
            {#snippet actions()}
              <Button onclick={() => void app.searchEverywhere()}>Search everywhere</Button>
            {/snippet}
          </EmptyState>
        {:else}
          <EmptyState title="No matches">Try fewer or shorter words.</EmptyState>
        {/if}
      {:else if app.selection.kind === 'allUnread'}
        <EmptyState title="All caught up">Enjoy the late-summer sun.</EmptyState>
      {:else}
        <EmptyState title="Nothing here">No Articles in this view.</EmptyState>
      {/if}
    </div>
  {:else if app.layout === 'cards'}
    <CardsLayout label={title} />
  {:else if app.layout === 'magazine'}
    <MagazineLayout label={title} />
  {:else if app.layout === 'gallery'}
    <GalleryLayout label={title} />
  {:else}
    <ListLayout label={title} />
  {/if}
</section>

<style>
  .list {
    position: relative;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    overflow: hidden;
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
    min-width: 0;
    align-items: center;
    gap: var(--space-inline-xs);
  }
  .search {
    flex: 1;
    min-width: 0;
    /* Grid Layouts span the window: the field must not stretch with them. */
    max-width: var(--article-list-search-max-width);
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
  .scope {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: var(--space-stack-2xs);
    max-width: 45%;
    min-height: 24px;
    padding-inline: var(--space-inset-sm);
    border: 0;
    border-radius: var(--radius-pill);
    background: var(--color-accent-subtle);
    color: var(--color-text-default);
    font: inherit;
    font-size: var(--font-size-caption);
    cursor: pointer;
  }
  .scope-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .search:focus-within {
    border-color: var(--color-border-focus);
  }
  input {
    flex: 1;
    width: 0;
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
    animation: spin var(--motion-duration-spin) linear infinite;
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
    z-index: var(--layer-list);
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
  .empty,
  .list > :global(.viewport) {
    min-height: 0;
  }
  .switcher {
    display: inline-flex;
    padding: 2px;
    border-radius: var(--radius-control);
    background: var(--color-surface-sunken);
  }
  .segment {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: calc(var(--control-height) - 4px);
    height: calc(var(--control-height) - 4px);
    border: 0;
    border-radius: calc(var(--radius-control) - 2px);
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .segment:hover {
    color: var(--color-text-default);
  }
  .segment[aria-pressed='true'] {
    background: var(--color-surface-raised);
    color: var(--color-text-default);
    box-shadow: var(--shadow-raised);
  }
  .note {
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
</style>
