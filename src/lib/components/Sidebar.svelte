<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import type { FeedNode } from '../api';
  import { app, type Selection } from '../state.svelte';
  import { applyTheme, type Theme } from '../tokens';
  import Icon, { type IconName } from './Icon.svelte';

  // Collapsed Folders are a per-machine view preference, not Library data.
  const COLLAPSED_KEY = 'feedrico.collapsedFolders';
  let collapsed = $state<Set<number>>(
    new Set(JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? '[]') as number[]),
  );
  function toggleFolder(id: number) {
    const next = new Set(collapsed);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    collapsed = next;
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...next]));
  }

  const THEME_KEY = 'feedrico.theme';
  const themes: Theme[] = ['system', 'light', 'dark'];
  let theme = $state<Theme>((localStorage.getItem(THEME_KEY) as Theme | null) ?? 'system');
  $effect(() => {
    applyTheme(theme);
    localStorage.setItem(THEME_KEY, theme);
  });

  const smart: { selection: Selection; label: string; icon: IconName; count: () => number }[] = [
    { selection: { kind: 'allUnread' }, label: 'All Unread', icon: 'inbox', count: () => app.sidebar?.allUnread ?? 0 },
    { selection: { kind: 'today' }, label: 'Today', icon: 'today', count: () => app.sidebar?.today ?? 0 },
    { selection: { kind: 'starred' }, label: 'Starred', icon: 'star', count: () => app.sidebar?.starred ?? 0 },
  ];

  async function importOpml() {
    const file = await open({
      title: 'Import OPML',
      filters: [{ name: 'OPML', extensions: ['opml', 'xml'] }],
    });
    if (typeof file === 'string') await app.importOpml(file);
  }

  async function exportOpml() {
    const file = await save({
      title: 'Export OPML',
      defaultPath: 'feedrico.opml',
      filters: [{ name: 'OPML', extensions: ['opml'] }],
    });
    if (file) await app.exportOpml(file);
  }

  const brokenTitle = (f: FeedNode) =>
    `${f.lastError}${f.lastSuccessAt ? ` · last worked ${new Date(f.lastSuccessAt * 1000).toLocaleString()}` : ''}`;
</script>

{#snippet feedRow(feed: FeedNode, nested: boolean)}
  <li>
    <button
      class="row"
      class:nested
      class:unread={feed.unread > 0}
      aria-current={app.isSelected({ kind: 'feed', id: feed.id }) ? 'page' : undefined}
      onclick={() => app.select({ kind: 'feed', id: feed.id })}
    >
      <span class="title">{feed.title}</span>
      {#if feed.lastError}
        <span class="broken" title={brokenTitle(feed)}><Icon name="warning" size={14} label="Feed has problems" /></span>
      {/if}
      {#if feed.unread}<span class="count">{feed.unread}</span>{/if}
    </button>
  </li>
{/snippet}

<nav class="sidebar" aria-label="Feeds">
  <header class="brand">
    <span class="wordmark">Feedrico</span>
  </header>

  <div class="scroll">
    <ul class="group">
      {#each smart as item (item.label)}
        <li>
          <button
            class="row smart"
            aria-current={app.isSelected(item.selection) ? 'page' : undefined}
            onclick={() => app.select(item.selection)}
          >
            <Icon name={item.icon} size={16} />
            <span class="title">{item.label}</span>
            {#if item.count()}<span class="count">{item.count()}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>

    {#if app.sidebar && (app.sidebar.folders.length || app.sidebar.feeds.length)}
      <ul class="group feeds">
        {#each app.sidebar.folders as folder (folder.id)}
          <li>
            <div class="folder-row">
              <button
                class="disclosure"
                aria-expanded={!collapsed.has(folder.id)}
                aria-label={collapsed.has(folder.id) ? `Expand ${folder.name}` : `Collapse ${folder.name}`}
                onclick={() => toggleFolder(folder.id)}
              >
                <span class="chevron" class:open={!collapsed.has(folder.id)}><Icon name="chevron" size={14} /></span>
              </button>
              <button
                class="row folder"
                class:unread={folder.unread > 0}
                aria-current={app.isSelected({ kind: 'folder', id: folder.id }) ? 'page' : undefined}
                onclick={() => app.select({ kind: 'folder', id: folder.id })}
              >
                <span class="title">{folder.name}</span>
                {#if folder.unread}<span class="count">{folder.unread}</span>{/if}
              </button>
            </div>
            {#if !collapsed.has(folder.id)}
              <ul>
                {#each folder.feeds as feed (feed.id)}
                  {@render feedRow(feed, true)}
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
        {#each app.sidebar.feeds as feed (feed.id)}
          {@render feedRow(feed, false)}
        {/each}
      </ul>
    {:else}
      <div class="onboarding">
        <p>No Feeds yet.</p>
        <button class="primary" onclick={() => (app.addFeedOpen = true)}><Icon name="plus" size={16} /> Add Feed…</button>
        <button onclick={importOpml}><Icon name="import" size={16} /> Import OPML…</button>
      </div>
    {/if}
  </div>

  <footer class="footer">
    <div class="library" title={app.library?.path}>
      <Icon name="library" size={14} />
      <span>{app.library?.portable ? 'Portable Library' : (app.library?.name ?? '')}</span>
    </div>
    <div class="tools">
      <button class="tool" title="Add Feed" aria-label="Add Feed" onclick={() => (app.addFeedOpen = true)}><Icon name="plus" size={16} /></button>
      <button class="tool" title="Import OPML" aria-label="Import OPML" onclick={importOpml}><Icon name="import" size={16} /></button>
      <button class="tool" title="Export OPML" aria-label="Export OPML" onclick={exportOpml}><Icon name="export" size={16} /></button>
      <button class="tool" title="Open another Library" aria-label="Open another Library" onclick={() => app.showWelcome()}><Icon name="folder" size={16} /></button>
      <button
        class="tool"
        title={`Theme: ${theme}`}
        aria-label={`Theme: ${theme}`}
        onclick={() => (theme = themes[(themes.indexOf(theme) + 1) % themes.length] ?? 'system')}
      ><Icon name="sun" size={16} /></button>
    </div>
  </footer>
</nav>

<style>
  .sidebar {
    display: grid;
    grid-template-rows: auto 1fr auto;
    min-height: 0;
    height: 100%;
    background: var(--color-surface-sunken);
    border-inline-end: 1px solid var(--color-border-subtle);
  }
  .brand {
    padding: var(--space-inset-lg) var(--space-inset-lg) var(--space-inset-sm);
  }
  .wordmark {
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-md);
    font-weight: var(--font-weight-heading);
    color: var(--color-text-accent);
    letter-spacing: -0.01em;
  }
  .scroll {
    overflow-y: auto;
    padding: var(--space-inset-sm);
    min-height: 0;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 1px;
  }
  .group + .group {
    margin-block-start: var(--space-stack-md);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-inline-sm);
    width: 100%;
    height: var(--sidebar-row-height);
    padding-inline: var(--sidebar-row-padding-inline);
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--color-text-muted);
    text-align: start;
    cursor: pointer;
    transition: background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .row.unread,
  .row.smart {
    color: var(--color-text-default);
  }
  .row:hover {
    background: var(--color-surface-hover);
  }
  .row[aria-current='page'] {
    background: var(--color-surface-selected);
    color: var(--color-text-default);
    font-weight: var(--font-weight-medium);
  }
  .row.nested {
    padding-inline-start: calc(var(--sidebar-row-padding-inline) + var(--space-inset-lg) + var(--space-inline-xs));
  }
  .row.folder {
    font-weight: var(--font-weight-medium);
  }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: var(--font-size-caption);
    font-variant-numeric: tabular-nums;
    color: var(--color-text-muted);
  }
  .broken {
    display: inline-flex;
    color: var(--color-status-warning-text);
  }
  .folder-row {
    display: flex;
    align-items: center;
  }
  .disclosure {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--space-inset-lg);
    height: var(--sidebar-row-height);
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .chevron {
    display: inline-flex;
    transition: rotate var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .chevron.open {
    rotate: 90deg;
  }
  .onboarding {
    display: grid;
    gap: var(--space-stack-sm);
    justify-items: start;
    padding: var(--space-inset-md) var(--sidebar-row-padding-inline);
    color: var(--color-text-muted);
  }
  .primary {
    display: inline-flex;
    align-items: center;
    gap: var(--space-inline-xs);
    height: var(--control-height);
    padding-inline: var(--space-inset-md);
    border: 0;
    border-radius: var(--radius-control);
    background: var(--color-accent-default);
    color: var(--color-text-on-accent);
    font-weight: var(--font-weight-strong);
    cursor: pointer;
  }
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-inline-sm);
    padding: var(--space-inset-sm) var(--space-inset-md);
    border-block-start: 1px solid var(--color-border-subtle);
  }
  .library {
    display: flex;
    align-items: center;
    gap: var(--space-inline-xs);
    min-width: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .library span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tools {
    display: flex;
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
</style>
