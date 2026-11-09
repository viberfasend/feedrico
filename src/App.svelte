<script lang="ts">
  import { onMount } from 'svelte';
  import AddFeed from './lib/components/AddFeed.svelte';
  import ArticleList from './lib/components/ArticleList.svelte';
  import Attention from './lib/components/Attention.svelte';
  import Reader from './lib/components/Reader.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Toasts from './lib/components/Toasts.svelte';
  import Welcome from './lib/components/Welcome.svelte';
  import { app } from './lib/state.svelte';
  import { fade, fly } from 'svelte/transition';

  /** Moves focus into the slide-over and back to where it came from when it closes. */
  function focusOnOpen(node: HTMLElement) {
    const previous = document.activeElement as HTMLElement | null;
    node.tabIndex = -1;
    node.focus({ preventScroll: true });
    return { destroy: () => previous?.focus({ preventScroll: true }) };
  }

  onMount(() => {
    void app.start();
  });

  // Keyboard-first reading (spec: "Keyboard"). Shortcuts never fire while typing.
  function onKeydown(event: KeyboardEvent) {
    if (app.phase !== 'ready' || event.metaKey || event.ctrlKey || event.altKey) return;
    const target = event.target as HTMLElement;
    if (target.closest('input, textarea, select, [contenteditable="true"]')) return;
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
      r: () => void app.startRefresh(true),
      a: () => (app.addFeedOpen = true),
      '/': () => document.getElementById('search')?.focus(),
    };
    const action = actions[event.key];
    if (action) {
      event.preventDefault();
      action();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if app.phase === 'welcome'}
  <Welcome />
{:else if app.phase === 'attention'}
  <Attention />
{:else if app.phase === 'ready'}
  <div class="shell" class:grid-layout={app.isGrid}>
    <Sidebar />
    <ArticleList />
    {#if !app.isGrid}
      <Reader />
    {/if}
  </div>
  {#if app.isGrid && app.readerOpen}
    <button class="backdrop" aria-label="Close reader" tabindex="-1" onclick={() => app.closeReader()} transition:fade={{ duration: 160 }}></button>
    <div class="slide-over" role="dialog" aria-modal="true" aria-label="Reader" transition:fly={{ x: 80, duration: 220 }} use:focusOnOpen>
      <Reader />
    </div>
  {/if}
{/if}

{#if app.addFeedOpen}
  <AddFeed />
{/if}
<Toasts />

<style>
  .shell.grid-layout {
    grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 4;
    border: 0;
    background: var(--color-media-scrim);
    opacity: 0.35;
    cursor: default;
  }
  .slide-over {
    position: fixed;
    inset-block: 0;
    inset-inline-end: 0;
    z-index: 5;
    width: min(var(--slide-over-width), var(--slide-over-max-width));
    box-shadow: var(--shadow-overlay);
    border-inline-start: 1px solid var(--color-border-default);
    outline: none;
  }
  .shell {
    display: grid;
    grid-template-columns: var(--sidebar-width) var(--article-list-width) minmax(0, 1fr);
    height: 100%;
    overflow: hidden;
  }
</style>
