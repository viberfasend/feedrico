<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import type { FeedNode, FolderNode } from '../api';
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

  // ---- Managing Feeds and Folders: context menu, inline rename, drag and drop ----------------
  type Target = { kind: 'feed'; feed: FeedNode } | { kind: 'folder'; folder: FolderNode };
  let menu = $state<{ x: number; y: number; target: Target } | null>(null);
  let editing = $state<{ kind: 'feed' | 'folder'; id: number } | null>(null);
  let confirm = $state<Target | null>(null);
  let dragging = $state<number | null>(null);
  let dropTarget = $state<number | 'top' | null>(null);

  function openMenu(event: MouseEvent, target: Target) {
    event.preventDefault();
    menu = { x: event.clientX, y: event.clientY, target };
  }
  function openMenuFromKey(event: KeyboardEvent, target: Target) {
    if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return;
    event.preventDefault();
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    menu = { x: box.left + 24, y: box.bottom, target };
  }
  const closeMenu = () => (menu = null);
  function startRename(target: Target) {
    closeMenu();
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
    closeMenu();
    await app.moveFeed(feedId, folderId);
  }
  async function confirmed() {
    const target = confirm;
    confirm = null;
    if (!target) return;
    if (target.kind === 'feed') await app.unsubscribe(target.feed.id);
    else await app.deleteFolder(target.folder.id);
  }
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
    `${f.lastError}${f.lastSuccessAt ? ` · last worked ${new Date(f.lastSuccessAt * 1000).toLocaleString()}` : ''}`;
</script>

{#snippet feedRow(feed: FeedNode, nested: boolean)}
  <li>
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
      oncontextmenu={(e) => openMenu(e, { kind: 'feed', feed })}
      onkeydown={(e) => openMenuFromKey(e, { kind: 'feed', feed })}
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
              <button
                class="row folder"
                class:unread={folder.unread > 0}
                aria-current={app.isSelected({ kind: 'folder', id: folder.id }) ? 'page' : undefined}
                class:drop={dropTarget === folder.id}
                ondragover={(e) => dragOver(e, folder.id)}
                ondragleave={() => (dropTarget = null)}
                ondrop={(e) => drop(e, folder.id)}
                oncontextmenu={(e) => openMenu(e, { kind: 'folder', folder })}
                onkeydown={(e) => openMenuFromKey(e, { kind: 'folder', folder })}
                onclick={() => app.select({ kind: 'folder', id: folder.id })}
              >
                {#if editing?.kind === 'folder' && editing.id === folder.id}
                  <input class="rename" aria-label="Folder name" value={folder.name} use:focusSelect
                    onblur={(e) => commitRename(e, folder.name)} onkeydown={renameKeys} onclick={(e) => e.stopPropagation()} />
                {:else}
                  <span class="title">{folder.name}</span>
                {/if}
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

<svelte:window onclick={closeMenu} onkeydown={(e) => e.key === 'Escape' && (menu = confirm = null)} />

{#if menu}
  {@const target = menu.target}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_interactive_supports_focus, a11y_click_events_have_key_events -->
  <div class="menu" role="menu" style:left="{menu.x}px" style:top="{menu.y}px" onclick={(e) => e.stopPropagation()}>
    <button role="menuitem" onclick={() => startRename(target)}>Rename</button>
    {#if target.kind === 'feed'}
      {@const feed = target.feed}
      <div class="menu-label">Move to</div>
      <button role="menuitem" onclick={() => moveTo(feed.id, null)}>No Folder</button>
      {#each app.sidebar?.folders ?? [] as folder (folder.id)}
        <button role="menuitem" onclick={() => moveTo(feed.id, folder.id)}>{folder.name}</button>
      {/each}
      <hr />
      <button role="menuitem" class="danger" onclick={() => { confirm = target; closeMenu(); }}>Unsubscribe…</button>
    {:else}
      <hr />
      <button role="menuitem" class="danger" onclick={() => { confirm = target; closeMenu(); }}>Delete Folder…</button>
    {/if}
  </div>
{/if}

{#if confirm}
  {@const target = confirm}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (confirm = null)}>
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title">
      {#if target.kind === 'feed'}
        <h2 id="confirm-title">Unsubscribe from {target.feed.title}?</h2>
        <p>Its Articles and cached images are removed from this Library. This can't be undone.</p>
      {:else}
        <h2 id="confirm-title">Delete the Folder {target.folder.name}?</h2>
        <p>Its Feeds move to the top level. No Feeds or Articles are removed.</p>
      {/if}
      <div class="actions">
        <button onclick={() => (confirm = null)}>Cancel</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="danger-solid" autofocus onclick={confirmed}>
          {target.kind === 'feed' ? 'Unsubscribe' : 'Delete Folder'}
        </button>
      </div>
    </div>
  </div>
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
  .menu {
    position: fixed;
    z-index: 30;
    display: grid;
    min-width: 12rem;
    padding: var(--space-inset-xs);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
  }
  .menu button {
    height: var(--sidebar-row-height);
    padding-inline: var(--space-inset-md);
    border: 0;
    border-radius: var(--radius-control);
    background: none;
    color: var(--color-text-default);
    text-align: start;
    cursor: pointer;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--color-surface-hover);
  }
  .menu-label {
    padding: var(--space-inset-sm) var(--space-inset-md) 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .menu hr {
    width: 100%;
    margin: var(--space-inset-xs) 0;
    border: 0;
    border-top: 1px solid var(--color-border-subtle);
  }
  .danger {
    color: var(--color-status-warning-text);
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, black 40%, transparent);
  }
  .dialog {
    width: min(28rem, calc(100vw - 2rem));
    padding: var(--space-inset-xl);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
  }
  .dialog h2 {
    margin: 0 0 var(--space-stack-sm);
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-md);
  }
  .dialog p {
    margin: 0 0 var(--space-stack-md);
    color: var(--color-text-muted);
  }
  .dialog .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-inline-sm);
  }
  .dialog .actions button {
    height: var(--control-height);
    padding-inline: var(--space-inset-lg);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    color: var(--color-text-default);
    cursor: pointer;
  }
  .dialog .actions .danger-solid {
    border-color: transparent;
    background: var(--color-accent-default);
    color: var(--color-text-on-accent);
    font-weight: var(--font-weight-strong);
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
