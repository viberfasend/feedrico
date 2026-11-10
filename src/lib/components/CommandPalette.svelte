<script lang="ts">
  import { rank, type Rankable } from '../palette';
  import { app } from '../state.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Input from '../ui/Input.svelte';
  import Icon, { type IconName } from './Icon.svelte';

  type Group = 'action' | 'folder' | 'feed';

  interface PaletteEntry extends Rankable {
    id: string;
    group: Group;
    icon: IconName;
    run: () => void;
  }

  let { open = $bindable(false) }: { open?: boolean } = $props();

  let query = $state('');
  let selected = $state(0);

  /** Rendered rows never exceed this, so 300 Feeds stay instant. */
  const MAX_ROWS = 50;

  const GROUP_LABEL: Record<Group, string> = {
    action: 'Actions',
    folder: 'Folders',
    feed: 'Feeds',
  };

  function actions(): PaletteEntry[] {
    return [
      { id: 'action:refresh', group: 'action', icon: 'refresh', label: 'Refresh all Feeds', run: () => void app.startRefresh(true) },
      { id: 'action:addFeed', group: 'action', icon: 'plus', label: 'Add Feed…', run: () => (app.addFeedOpen = true) },
      { id: 'action:importOpml', group: 'action', icon: 'import', label: 'Import OPML…', run: () => void app.chooseImportOpml() },
      { id: 'action:exportOpml', group: 'action', icon: 'export', label: 'Export OPML…', run: () => void app.chooseExportOpml() },
      { id: 'action:markAllRead', group: 'action', icon: 'check', label: 'Mark all as read', run: () => void app.markAllRead() },
      { id: 'action:theme', group: 'action', icon: 'sun', label: `Toggle theme (now ${app.theme})`, run: () => app.cycleTheme() },
      { id: 'action:openLibrary', group: 'action', icon: 'library', label: 'Open another Library…', run: () => app.showWelcome() },
      { id: 'action:updates', group: 'action', icon: 'update', label: 'Check for updates', run: () => void app.checkForUpdates() },
    ];
  }

  function folders(): PaletteEntry[] {
    return (app.sidebar?.folders ?? []).map((f) => ({
      id: `folder:${f.id}`,
      group: 'folder' as const,
      icon: 'folder' as const,
      label: f.name,
      run: () => app.select({ kind: 'folder', id: f.id }),
    }));
  }

  function feeds(): PaletteEntry[] {
    const sidebar = app.sidebar;
    const all = [...(sidebar?.folders ?? []).flatMap((f) => f.feeds), ...(sidebar?.feeds ?? [])];
    return all.map((f) => ({
      id: `feed:${f.id}`,
      group: 'feed' as const,
      icon: 'circle' as const,
      label: f.title,
      keywords: f.siteUrl ?? undefined,
      run: () => app.select({ kind: 'feed', id: f.id }),
    }));
  }

  /** One ranked list; empty query keeps the group order actions → Folders → Feeds. */
  let visible = $derived(
    [...rank(query, actions()), ...rank(query, folders()), ...rank(query, feeds())].slice(0, MAX_ROWS),
  );

  // Fresh query and selection each time the palette opens; no IPC, everything is already loaded.
  let wasOpen = false;
  $effect(() => {
    if (open && !wasOpen) {
      query = '';
      selected = 0;
      const t = setTimeout(() => document.getElementById('palette-input')?.focus(), 0);
      wasOpen = true;
      return () => clearTimeout(t);
    }
    if (!open) wasOpen = false;
  });

  $effect(() => {
    if (selected >= visible.length) selected = Math.max(0, visible.length - 1);
  });

  // Keep the highlighted row in view.
  $effect(() => {
    void selected;
    document.getElementById(`palette-option-${selected}`)?.scrollIntoView({ block: 'nearest' });
  });

  function onInputKeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      selected = (selected + 1) % Math.max(1, visible.length);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      selected = (selected - 1 + Math.max(1, visible.length)) % Math.max(1, visible.length);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const entry = visible[selected];
      if (entry) run(entry);
    }
  }

  function run(entry: PaletteEntry) {
    open = false;
    entry.run();
  }
</script>

<Dialog bind:open title="Command palette" class="palette-dialog">
  <Input
    bind:value={query}
    id="palette-input"
    type="text"
    placeholder="Jump to a Feed or Folder, or run an action…"
    autocomplete="off"
    spellcheck={false}
    role="combobox"
    aria-expanded="true"
    aria-controls="palette-list"
    aria-activedescendant={visible[selected] ? `palette-option-${selected}` : undefined}
    onkeydown={onInputKeydown}
  />
  <div class="list" id="palette-list" role="listbox" aria-label="Commands, Folders and Feeds">
    {#each visible as entry, i (entry.id)}
      {#if i === 0 || visible[i - 1]!.group !== entry.group}
        <div class="group-label" role="presentation">{GROUP_LABEL[entry.group]}</div>
      {/if}
      <button
        type="button"
        id="palette-option-{i}"
        class="option"
        class:selected={i === selected}
        role="option"
        aria-selected={i === selected}
        onclick={() => run(entry)}
        onmousemove={() => (selected = i)}
      >
        <Icon name={entry.icon} size={14} />
        <span class="option-label">{entry.label}</span>
      </button>
    {:else}
      <div class="empty">No matches</div>
    {/each}
  </div>
</Dialog>

<style>
  /* Passed through to the Dialog component's element, so it must be global. */
  :global(.palette-dialog) {
    inset-block-start: 10vh;
    margin-block: 0;
    padding-block: var(--space-inset-md);
  }
  .list {
    display: grid;
    gap: 1px;
    max-height: 50vh;
    overflow-y: auto;
    margin-block-start: var(--space-inset-sm);
  }
  .group-label {
    padding: var(--space-inset-xs) var(--space-inset-md) 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .option {
    display: flex;
    align-items: center;
    gap: var(--space-inline-sm);
    height: var(--menu-item-height);
    padding-inline: var(--space-inset-md);
    border: 0;
    border-radius: var(--radius-control);
    background: none;
    color: var(--color-text-default);
    text-align: start;
    cursor: pointer;
  }
  .option.selected {
    background: var(--color-surface-hover);
  }
  .option-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: var(--space-inset-md);
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
</style>
