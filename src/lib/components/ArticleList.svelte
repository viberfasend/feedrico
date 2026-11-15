<script lang="ts">
  import { onMount } from 'svelte';
  import { LAYOUTS, type Layout } from '../api';
  import { assist } from '../assistant-state.svelte';
  import { app } from '../state.svelte';
  import Button from '../ui/Button.svelte';
  import Menu, { type MenuItem } from '../ui/Menu.svelte';
  import AssistantViewPanel from './AssistantViewPanel.svelte';
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

  const sortLabel = $derived(app.sort === 'newestFirst' ? 'newest first' : 'oldest first');
  const viewLabel = $derived(`View options: ${layoutMeta[app.layout].label}, ${sortLabel}`);
  const viewItems = $derived<MenuItem[]>([
    ...LAYOUTS.map((layout, i): MenuItem => ({
      label: layoutMeta[layout].label,
      checked: app.layout === layout,
      headingBefore: i === 0 ? 'Layout (v cycles)' : undefined,
      onclick: () => void app.setLayout(layout),
    })),
    {
      label: 'Newest first',
      checked: app.sort === 'newestFirst',
      separatorBefore: true,
      headingBefore: 'Sort',
      onclick: () => app.setSort('newestFirst'),
    },
    { label: 'Oldest first', checked: app.sort === 'oldestFirst', onclick: () => app.setSort('oldestFirst') },
    // The Assistant's View actions (#120, #121), only with a setup. They live here rather than
    // in a button of their own, so the header keeps room for the search field at 880 px.
    ...(assist.viewAvailable
      ? assist.viewEntries.map(
          (e, i): MenuItem => ({
            label: e.menuLabel,
            disabled: app.total === 0,
            separatorBefore: i === 0,
            headingBefore: i === 0 ? 'Assistant' : undefined,
            onclick: () => assist.start(e),
          }),
        )
      : []),
  ]);

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
  /** The name of the scope the current search runs in (an Assistant-picked one wins, #122). */
  const scopeName = $derived.by(() => {
    const s = app.searchScope;
    if (!s) return viewName;
    const sidebar = app.sidebar;
    if (s.kind === 'folder') return sidebar?.folders.find((f) => f.id === s.id)?.name ?? 'Folder';
    const feeds = [...(sidebar?.feeds ?? []), ...(sidebar?.folders.flatMap((f) => f.feeds) ?? [])];
    return feeds.find((f) => f.id === s.id)?.title ?? 'Feed';
  });

  const hasFeeds = $derived((app.library?.feedCount ?? 0) > 0);
  const refreshing = $derived(app.refresh !== null && !app.refresh.finished);
  /** The date range an Assistant search picked (#122), shown so the filter is never hidden. */
  const dateRange = $derived.by(() => {
    const a = app.searchAssisted;
    if (!app.search.trim() || !a || (a.since == null && a.until == null)) return null;
    const fmt = (t: number) => new Date(t * 1000).toLocaleDateString(undefined, { dateStyle: 'medium' });
    if (a.since != null && a.until != null) return `from ${fmt(a.since)} – ${fmt(a.until)}`;
    return a.since != null ? `since ${fmt(a.since)}` : `until ${fmt(a.until!)}`;
  });
  const showAssistantPanel = $derived(assist.viewRun !== null);

  // The search trigger and the View menu's Assistant actions need the machine-local setup, even
  // with no Article open (the reader's panel loads it only then); reading it is cheap and local.
  onMount(() => void assist.loadSetup());

  let searchTimer: ReturnType<typeof setTimeout>;
  function onSearch(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => app.setSearch(value), 150);
  }

  /** The separate Assistant trigger (#122): the field's text becomes the question. */
  async function onAskAssistant() {
    const input = document.getElementById('search') as HTMLInputElement | null;
    const question = (input?.value ?? app.search).trim();
    if (!question) {
      input?.focus();
      return;
    }
    await assist.askSearch(question);
  }


</script>

<section class="list" class:has-panel={showAssistantPanel} id="pane-list" aria-label="Articles" tabindex="-1">
  <header class="header">
    <div class="heading">
      <h1 title={title}>{title}</h1>
      {#if !app.hideCounts}<span class="total">{app.total.toLocaleString()}</span>{/if}
      {#if dateRange}
        <button
          class="dates"
          title="Remove the date range"
          aria-label={`Only Articles ${dateRange}. Remove the date range`}
          onclick={() => void app.clearSearchDates()}
        ><span class="scope-name" title={dateRange}>{dateRange.replace(/^from /, '')}</span><Icon name="close" size={12} /></button>
      {/if}
    </div>
    <div class="controls">
      <label class="search">
        <Icon name="search" size={15} />
        {#if showScope}
          <button
            class="scope"
            title="Search everywhere instead"
            aria-label={`Searching in ${scopeName}. Search everywhere instead`}
            onmousedown={(e) => e.preventDefault()}
            onclick={() => void app.searchEverywhere()}
          ><span class="scope-name" title="in {scopeName}">in {scopeName}</span><Icon name="close" size={12} /></button>
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
      {#if assist.setup?.disclosureAccepted}
        <button
          class="tool"
          id="assistant-search"
          class:spinning={assist.searchRun === 'running'}
          title="Ask the Assistant to search"
          aria-label="Ask the Assistant to search"
          aria-busy={assist.searchRun === 'running'}
          disabled={assist.searchRun === 'running'}
          onclick={() => void onAskAssistant()}
        ><Icon name="sparkles" size={16} /></button>
      {/if}
      <Menu items={viewItems} label={viewLabel} align="end" class="tool">
        <Icon name={layoutMeta[app.layout].icon} size={16} />
      </Menu>
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

  {#if showAssistantPanel}
    <AssistantViewPanel />
  {/if}

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
          <EmptyState title={`No matches in ${scopeName}`}>
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
  /* With an Assistant panel above the list (#120/#121), the list gets the remaining row. */
  .list.has-panel {
    /* The panel row takes what the panel needs, at most 45% of the pane; the panel scrolls
       inside it. A percentage max-height on the panel itself would resolve against its own
       content-sized row and leave a gap below it. */
    grid-template-rows: auto fit-content(45%) minmax(0, 1fr);
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
    /* The date chip of an Assistant search moves to its own line rather than squeezing the
       title at the window's minimum width. */
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-stack-2xs) var(--space-inline-sm);
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
  .dates {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: var(--space-stack-2xs);
    min-width: 0;
    max-width: 100%;
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
  .tool,
  .controls :global(.menu-trigger.tool) {
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
  .tool:hover,
  .controls :global(.menu-trigger.tool:hover) {
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
  .note {
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
</style>
