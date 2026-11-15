<script lang="ts">
  import { onMount } from 'svelte';
  import AddFeed from './lib/components/AddFeed.svelte';
  import ArticleList from './lib/components/ArticleList.svelte';
  import Attention from './lib/components/Attention.svelte';
  import BrokenFeeds from './lib/components/BrokenFeeds.svelte';
  import CommandPalette from './lib/components/CommandPalette.svelte';
  import Reader from './lib/components/Reader.svelte';
  import Settings from './lib/components/Settings.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Toasts from './lib/components/Toasts.svelte';
  import Welcome from './lib/components/Welcome.svelte';
  import { pageReader } from './lib/paging';
  import { panes } from './lib/pane-state.svelte';
  import { layoutPanes, listRange, sidebarRange, slideOverRange, type PaneLimits } from './lib/panes';
  import { app } from './lib/state.svelte';
  import Splitter from './lib/ui/Splitter.svelte';
  import { tokens } from './lib/tokens/runtime.svelte';
  import { fade, fly } from 'svelte/transition';

  /**
   * Moves focus into the slide-over on open and back to the selected Article row on close, so
   * Esc returns focus to where the reader came from.
   */
  function focusOnOpen(node: HTMLElement) {
    const previous = document.activeElement as HTMLElement | null;
    node.tabIndex = -1;
    node.focus({ preventScroll: true });
    return {
      destroy: () => {
        const row = document.querySelector(
          '#pane-list [role="option"][aria-selected="true"]',
        ) as HTMLElement | null;
        (row ?? previous)?.focus({ preventScroll: true });
      },
    };
  }

  /** F6 / Shift+F6 move between the panes: sidebar → Article list → reader. */
  const PANE_IDS = ['pane-sidebar', 'pane-list', 'pane-reader'];
  function cyclePanes(forward: boolean) {
    const targets = PANE_IDS.filter((id) => id !== 'pane-sidebar' || !panes.sidebarHidden)
      .map((id) => document.getElementById(id))
      .filter((el): el is HTMLElement => el !== null);
    if (!targets.length) return;
    const current = targets.findIndex(
      (el) => el === document.activeElement || el.contains(document.activeElement),
    );
    const next =
      current === -1 ? 0 : (current + (forward ? 1 : -1) + targets.length) % targets.length;
    targets[next]?.focus({ preventScroll: true });
  }

  // Dev-only component gallery at #gallery; statically false in production builds, so the
  // dynamic import below is tree-shaken out of the bundle.
  const showGallery = import.meta.env.DEV && location.hash === '#gallery';

  onMount(() => {
    if (showGallery) return;
    // Perf mode: the shell injects this flag only when FEEDRICO_PERF_LIBRARY is set.
    app.perf = '__FEEDRICO_PERF__' in window;
    const started = app.start();
    if (app.perf) void started.then(() => import('./lib/perf').then((m) => m.perfSession(app)));
  });

  let paletteOpen = $state(false);

  // Pane sizes: preferences from pane-state, bounds from tokens, clamped to the window.
  let windowWidth = $state(window.innerWidth);
  // The shell's own width (0 until measured): the panes share what the shell actually has.
  let shellWidth = $state(0);
  let available = $derived(shellWidth || windowWidth);
  let limits = $derived<PaneLimits>({
    sidebarMin: tokens.px('--sidebar-min-width'),
    sidebarMax: tokens.px('--sidebar-max-width'),
    listMin: tokens.px('--article-list-min-width'),
    listMax: tokens.px('--article-list-max-width'),
    readerMin: tokens.px('--reader-min-width'),
  });
  let sidebarDefault = $derived(tokens.px('--sidebar-width'));
  let listDefault = $derived(tokens.px('--article-list-width'));
  let slideOverDefault = $derived(tokens.px('--slide-over-width'));
  // The token is a percentage ("88%"), so tokens.px would read it as 88.
  const slideOverFraction =
    parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--slide-over-max-width')) / 100 || 0.88;
  let widths = $derived(
    layoutPanes(
      {
        sidebar: panes.sidebar ?? sidebarDefault,
        list: panes.list ?? listDefault,
        sidebarHidden: panes.sidebarHidden,
        grid: app.isGrid,
        available,
      },
      limits,
    ),
  );
  let sidebarBounds = $derived(sidebarRange(app.isGrid, available, limits));
  let listBounds = $derived(listRange(widths.sidebar, available, limits));
  let slideOverBounds = $derived(slideOverRange(windowWidth, slideOverFraction, limits));
  let slideOverWidth = $derived(
    Math.min(Math.max(panes.slideOver ?? slideOverDefault, slideOverBounds.min), slideOverBounds.max),
  );

  // Keyboard-first reading (spec: "Keyboard"). Shortcuts never fire while typing.
  function onKeydown(event: KeyboardEvent) {
    // Ctrl/⌘+K toggles the command palette, even from inside the search field, but not while
    // another dialog is open.
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      if (!app.addFeedOpen) paletteOpen = !paletteOpen;
      return;
    }
    // Ctrl/⌘+, toggles the Settings dialog.
    if ((event.metaKey || event.ctrlKey) && event.key === ',') {
      event.preventDefault();
      if (app.settingsOpen) app.settingsOpen = false;
      else if (!app.addFeedOpen) {
        paletteOpen = false;
        void app.openSettings();
      }
      return;
    }
    if (app.phase !== 'ready' || event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.key === 'F6') {
      event.preventDefault();
      cyclePanes(!event.shiftKey);
      return;
    }
    const target = event.target;
    if (target instanceof HTMLElement && target.closest('input, textarea, select, [contenteditable="true"]')) return;
    const actions: Record<string, () => void> = {
      j: () => app.move(1),
      k: () => app.move(-1),
      ArrowDown: () => app.moveSpatial('down'),
      ArrowUp: () => app.moveSpatial('up'),
      ArrowLeft: () => app.moveSpatial('left'),
      ArrowRight: () => app.moveSpatial('right'),
      v: () => app.cycleLayout(),
      Enter: () => app.isGrid && !app.readerOpen && app.openReader(),
      Escape: () => app.closeReader(),
      s: () => void app.toggleStar(),
      m: () => void app.toggleRead(),
      o: () => app.openInBrowser(),
      b: () => void app.saveToFerrico(),
      r: () => void app.startRefresh(true),
      a: () => (app.addFeedOpen = true),
      '/': () => document.getElementById('search')?.focus(),
    };
    if (event.key === ' ') {
      // On a button or link Space is that control's own key.
      if (target instanceof HTMLElement && target.closest('button, a, summary, [role="tab"], [role="menuitem"]')) return;
      event.preventDefault();
      const reader = app.article ? document.getElementById('pane-reader') : null;
      if (!reader) {
        if (!event.shiftKey) void app.move(1);
      } else if (pageReader(reader, !event.shiftKey, reader.querySelector<HTMLElement>('[role="toolbar"]')?.offsetHeight ?? 0) === 'end') {
        void app.move(1);
      }
      return;
    }
    const action = actions[event.key];
    if (action) {
      event.preventDefault();
      action();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} bind:innerWidth={windowWidth} />

{#if showGallery}
  {#await import('./lib/components/Gallery.svelte') then { default: Gallery }}
    <Gallery />
  {/await}
{:else if app.phase === 'welcome'}
  <Welcome />
{:else if app.phase === 'attention'}
  <Attention />
{:else if app.phase === 'ready'}
  <main
    class="shell"
    class:grid-layout={app.isGrid}
    bind:clientWidth={shellWidth}
    style="--sidebar-width: {widths.sidebar}px; --article-list-width: {widths.list}px"
  >
    <div class="sidebar-slot" class:hidden={panes.sidebarHidden}>
      <Sidebar />
    </div>
    <ArticleList />
    {#if !app.isGrid}
      <Reader />
    {/if}
    <Splitter
      label="Resize sidebar"
      controls="pane-sidebar"
      value={widths.sidebar}
      min={sidebarBounds.min}
      max={sidebarBounds.max}
      style="left: calc({widths.sidebar}px - var(--splitter-hit-size) / 2)"
      onchange={(px) => panes.set('sidebar', px)}
      onreset={() => panes.set('sidebar', null)}
      ontoggle={() => panes.toggleSidebar()}
    />
    {#if !app.isGrid}
      <Splitter
        label="Resize Article list"
        controls="pane-list"
        value={widths.list}
        min={listBounds.min}
        max={listBounds.max}
        style="left: calc({widths.sidebar + widths.list}px - var(--splitter-hit-size) / 2)"
        onchange={(px) => panes.set('list', px)}
        onreset={() => panes.set('list', null)}
      />
    {/if}
  </main>
  {#if app.isGrid && app.readerOpen}
    <button class="backdrop" aria-label="Close reader" tabindex="-1" onclick={() => app.closeReader()} transition:fade={{ duration: tokens.px('--motion-duration-fast') }}></button>
    <div class="slide-over" style="--slide-over-width: {slideOverWidth}px" role="dialog" aria-modal="true" aria-label="Reader" transition:fly={{ x: 80, duration: tokens.px('--motion-duration-base') }} use:focusOnOpen>
      <Reader />
      <Splitter
        label="Resize reader"
        controls="pane-reader"
        inverted
        value={slideOverWidth}
        min={slideOverBounds.min}
        max={slideOverBounds.max}
        style="inset-inline-start: calc(var(--splitter-hit-size) / -2)"
        onchange={(px) => panes.set('slideOver', px)}
        onreset={() => panes.set('slideOver', null)}
      />
    </div>
  {/if}
{/if}

{#if app.addFeedOpen && !showGallery}
  <AddFeed />
{/if}
{#if app.settingsOpen && !showGallery}
  <Settings />
{/if}
{#if app.brokenFeedsOpen && !showGallery}
  <BrokenFeeds />
{/if}
{#if !showGallery}
  <CommandPalette bind:open={paletteOpen} />
  <Toasts />
{/if}

<style>
  .shell.grid-layout {
    grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: var(--layer-backdrop);
    border: 0;
    background: var(--color-media-scrim);
    opacity: 0.35;
    cursor: default;
  }
  .slide-over {
    position: fixed;
    inset-block: 0;
    inset-inline-end: 0;
    z-index: var(--layer-slide-over);
    width: var(--slide-over-width);
    box-shadow: var(--shadow-overlay);
    border-inline-start: 1px solid var(--color-border-default);
    outline: none;
  }
  .shell {
    display: grid;
    grid-template-columns: var(--sidebar-width) var(--article-list-width) minmax(0, 1fr);
    position: relative;
    height: 100%;
    overflow: hidden;
  }
  .sidebar-slot {
    display: grid;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  /* visibility removes the hidden sidebar from the accessibility tree and the tab order. */
  .sidebar-slot.hidden {
    visibility: hidden;
  }
</style>
