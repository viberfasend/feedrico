<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import type { FeedNode, FolderNode } from '../api';
  import { app, type Selection } from '../state.svelte';
  import Button from '../ui/Button.svelte';
  import ContextMenu, { type MenuItem } from '../ui/ContextMenu.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import Menu from '../ui/Menu.svelte';
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

  const smart: { selection: Selection; label: string; icon: IconName; count: () => number }[] = [
    { selection: { kind: 'allUnread' }, label: 'All Unread', icon: 'inbox', count: () => app.sidebar?.allUnread ?? 0 },
    { selection: { kind: 'today' }, label: 'Today', icon: 'today', count: () => app.sidebar?.today ?? 0 },
    { selection: { kind: 'starred' }, label: 'Starred', icon: 'star', count: () => app.sidebar?.starred ?? 0 },
  ];

  async function backUpLibrary() {
    const folder = await open({ title: 'Back up Library to an empty folder', directory: true });
    if (typeof folder === 'string') await app.exportLibrary(folder);
  }

  // ---- Managing Feeds and Folders: context menu, inline rename, drag and drop ----------------
  type Target = { kind: 'feed'; feed: FeedNode } | { kind: 'folder'; folder: FolderNode };
  let editing = $state<{ kind: 'feed' | 'folder'; id: number } | null>(null);
  let confirm = $state<Target | null>(null);
  let dragging = $state<number | null>(null);
  let dropTarget = $state<number | 'top' | null>(null);

  /** The context-menu entries for a Feed or Folder row. */
  function menuItems(target: Target): MenuItem[] {
    const items: MenuItem[] = [{ label: 'Rename', onclick: () => startRename(target) }];
    if (target.kind === 'feed') {
      items.push({ label: 'No Folder', headingBefore: 'Move to', onclick: () => void moveTo(target.feed.id, null) });
      for (const folder of app.sidebar?.folders ?? []) {
        items.push({ label: folder.name, onclick: () => void moveTo(target.feed.id, folder.id) });
      }
      items.push({ label: 'Unsubscribe…', danger: true, separatorBefore: true, onclick: () => (confirm = target) });
    } else {
      items.push({ label: 'Delete Folder…', danger: true, separatorBefore: true, onclick: () => (confirm = target) });
    }
    return items;
  }
  function startRename(target: Target) {
    editing = target.kind === 'feed' ? { kind: 'feed', id: target.feed.id } : { kind: 'folder', id: target.folder.id };
  }
  async function commitRename(event: Event, current: string) {
    const input = event.currentTarget as HTMLInputElement;
    const edit = editing;
    editing = null;
    const value = input.value.trim();
    if (!edit || !value || value === current) return;
    if (edit.kind === 'feed') await app.renameFeed(edit.id, value);
    else await app.renameFolder(edit.id, value);
  }
  function renameKeys(event: KeyboardEvent) {
    const input = event.currentTarget as HTMLInputElement;
    if (event.key === 'Enter') input.blur();
    else if (event.key === 'Escape') {
      event.stopPropagation();
      input.value = input.defaultValue;
      input.blur();
    }
  }
  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
  async function moveTo(feedId: number, folderId: number | null) {
    await app.moveFeed(feedId, folderId);
  }
  async function confirmed(unsubscribeFeeds: boolean) {
    const target = confirm;
    confirm = null;
    if (!target) return;
    if (target.kind === 'feed') await app.unsubscribe(target.feed.id);
    else await app.deleteFolder(target.folder.id, unsubscribeFeeds);
  }
  const moreItems = $derived<MenuItem[]>([
    { label: 'Import OPML…', onclick: () => void app.chooseImportOpml() },
    { label: 'Export OPML…', onclick: () => void app.chooseExportOpml() },
    {
      label: app.exportProgress ?? 'Export / back up Library…',
      disabled: app.exportProgress !== null,
      onclick: () => void backUpLibrary(),
    },
    { label: 'Settings…', onclick: () => void app.openSettings(), separatorBefore: true },
    { label: 'Open another Library…', onclick: () => app.showWelcome() },
    { label: 'Check for updates', onclick: () => void app.checkForUpdates() },
  ]);
  const feedsLabel = (n: number) => (n === 1 ? '1 Feed' : `${n} Feeds`);
  async function drop(event: DragEvent, folderId: number | null) {
    event.preventDefault();
    const id = dragging;
    dragging = dropTarget = null;
    if (id !== null) await app.moveFeed(id, folderId);
  }
  function dragOver(event: DragEvent, target: number | 'top') {
    if (dragging === null) return;
    event.preventDefault();
    dropTarget = target;
  }

  const brokenTitle = (f: FeedNode) =>
    `${f.lastError} · ${f.lastSuccessAt ? `last worked ${new Date(f.lastSuccessAt * 1000).toLocaleString()}` : 'has never worked'}`;
</script>

{#snippet feedRow(feed: FeedNode, nested: boolean)}
  <li>
    <ContextMenu items={menuItems({ kind: 'feed', feed })}>
      <button
        class="row"
        class:nested
        class:unread={feed.unread > 0}
        aria-current={app.isSelected({ kind: 'feed', id: feed.id }) ? 'page' : undefined}
        draggable={editing === null}
        ondragstart={(e) => {
          dragging = feed.id;
          e.dataTransfer?.setData('text/plain', feed.title);
        }}
        ondragend={() => (dragging = dropTarget = null)}
        onclick={() => app.select({ kind: 'feed', id: feed.id })}
      >
        {#if editing?.kind === 'feed' && editing.id === feed.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input class="rename" aria-label="Feed name" value={feed.title} use:focusSelect
            onblur={(e) => commitRename(e, feed.title)} onkeydown={renameKeys} onclick={(e) => e.stopPropagation()} />
        {:else}
          <span class="title">{feed.title}</span>
        {/if}
        {#if feed.lastError}
          <span class="broken" title={brokenTitle(feed)}><Icon name="warning" size={14} label="Feed has problems" /></span>
        {/if}
        {#if feed.unread}
          <span class="count">
            <span class="sr-only">{app.hideCounts ? 'has unread Articles' : `${feed.unread} unread`}</span>
            <span aria-hidden="true">{feed.unread}</span>
          </span>
        {/if}
      </button>
    </ContextMenu>
  </li>
{/snippet}

<nav class="sidebar" id="pane-sidebar" aria-label="Feeds" tabindex="-1">
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
            {#if item.count() && !(app.hideCounts && item.selection.kind === 'starred')}
              <span class="count">
                <span class="sr-only">{app.hideCounts ? 'has unread Articles' : `${item.count()} ${item.selection.kind === 'starred' ? 'starred' : 'unread'}`}</span>
                <span aria-hidden="true">{item.count()}</span>
              </span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>

    {#if app.sidebar && (app.sidebar.folders.length || app.sidebar.feeds.length)}
      <ul
        class="group feeds"
        class:drop={dropTarget === 'top'}
        ondragover={(e) => dragOver(e, 'top')}
        ondragleave={() => (dropTarget = null)}
        ondrop={(e) => drop(e, null)}
      >
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
              <ContextMenu items={menuItems({ kind: 'folder', folder })}>
                <button
                  class="row folder"
                  class:unread={folder.unread > 0}
                  aria-current={app.isSelected({ kind: 'folder', id: folder.id }) ? 'page' : undefined}
                  class:drop={dropTarget === folder.id}
                  ondragover={(e) => dragOver(e, folder.id)}
                  ondragleave={() => (dropTarget = null)}
                  ondrop={(e) => drop(e, folder.id)}
                  onclick={() => app.select({ kind: 'folder', id: folder.id })}
                >
                  {#if editing?.kind === 'folder' && editing.id === folder.id}
                    <input class="rename" aria-label="Folder name" value={folder.name} use:focusSelect
                      onblur={(e) => commitRename(e, folder.name)} onkeydown={renameKeys} onclick={(e) => e.stopPropagation()} />
                  {:else}
                    <span class="title">{folder.name}</span>
                  {/if}
                  {#if folder.unread}
                    <span class="count">
                      <span class="sr-only">{app.hideCounts ? 'has unread Articles' : `${folder.unread} unread`}</span>
                      <span aria-hidden="true">{folder.unread}</span>
                    </span>
                  {/if}
                </button>
              </ContextMenu>
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
        <Button variant="primary" onclick={() => (app.addFeedOpen = true)}><Icon name="plus" size={16} /> Add Feed…</Button>
        <Button onclick={() => void app.chooseImportOpml()}><Icon name="import" size={16} /> Import Feeds…</Button>
      </div>
    {/if}
  </div>

  <footer class="footer">
    <div class="library" title={app.library?.path}>
      <Icon name="library" size={14} />
      <span>{app.library?.portable ? 'Portable Library' : (app.library?.name ?? '')}</span>
    </div>
    <div class="tools">
      <IconButton label="Add Feed" icon="plus" onclick={() => (app.addFeedOpen = true)} />
      <IconButton label={`Theme: ${app.theme}`} icon="sun" onclick={() => app.cycleTheme()} />
      <!-- The sidebar is a fixed 260 px: occasional actions live here (and in the palette). -->
      <Menu items={moreItems} label="More actions" align="end" class="more">
        <Icon name="more" size={16} />
      </Menu>
    </div>
  </footer>
</nav>

{#if confirm}
  {@const target = confirm}
  <Dialog
    open={true}
    onOpenChange={(o) => { if (!o) confirm = null; }}
    title={target.kind === 'feed' ? `Unsubscribe from ${target.feed.title}?` : `Delete the Folder ${target.folder.name}?`}
  >
    <p class="confirm-text">
      {#if target.kind === 'feed'}
        Its Articles and cached images are removed from this Library. This can't be undone.
      {:else if target.folder.feeds.length === 0}
        The Folder is empty.
      {:else}
        It contains {feedsLabel(target.folder.feeds.length)}. Move them to the top level, or
        unsubscribe from them: their Articles and cached images are removed from this Library.
        Unsubscribing can't be undone.
      {/if}
    </p>
    <!-- Cancel comes first and gets focus on open, so Enter never removes anything. -->
    <div class="actions">
      <Button onclick={() => (confirm = null)}>Cancel</Button>
      {#if target.kind === 'feed'}
        <Button variant="primary" onclick={() => confirmed(true)}>Unsubscribe</Button>
      {:else if target.folder.feeds.length === 0}
        <Button variant="primary" onclick={() => confirmed(false)}>Delete Folder</Button>
      {:else}
        <Button onclick={() => confirmed(false)}>Move Feeds to top level</Button>
        <Button variant="primary" onclick={() => confirmed(true)}>
          Unsubscribe from {feedsLabel(target.folder.feeds.length)}
        </Button>
      {/if}
    </div>
  </Dialog>
{/if}

<style>
  .rename {
    flex: 1;
    min-width: 0;
    height: calc(var(--sidebar-row-height) - 6px);
    padding-inline: var(--space-inset-sm);
    border: 1px solid var(--color-accent-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-default);
    color: var(--color-text-default);
    font: inherit;
  }
  .drop {
    background: var(--color-surface-selected);
    outline: 1px dashed var(--color-accent-default);
  }
  .confirm-text {
    margin: 0 0 var(--space-stack-md);
    color: var(--color-text-muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-inline-sm);
  }
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
    overscroll-behavior-y: none;
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
    /* 1.4.11: the current View needs a 3:1 indicator, not just a background change. */
    box-shadow: inset 3px 0 0 var(--color-border-focus);
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
  /* Unread counts hidden (a Library setting): a quiet dot says "something new", no number. */
  :global([data-unread-counts='hide']) .count [aria-hidden='true'] {
    display: none;
  }
  :global([data-unread-counts='hide']) .count::after {
    content: '';
    display: block;
    width: 6px;
    height: 6px;
    border-radius: var(--radius-pill);
    background: var(--color-unread-indicator);
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
    width: var(--sidebar-row-height);
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
  /* The More trigger lives in Menu.svelte; match the footer's icon buttons. */
  .tools :global(.more) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--icon-button-size);
    height: var(--icon-button-size);
    border: 0;
    border-radius: var(--radius-control);
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .tools :global(.more:hover),
  .tools :global(.more[data-state='open']) {
    background: var(--color-surface-hover);
    color: var(--color-text-default);
  }
</style>
