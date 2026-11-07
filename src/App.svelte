<script lang="ts">
  import { onMount } from 'svelte';
  import ArticleList from './lib/components/ArticleList.svelte';
  import Attention from './lib/components/Attention.svelte';
  import Reader from './lib/components/Reader.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Toasts from './lib/components/Toasts.svelte';
  import Welcome from './lib/components/Welcome.svelte';
  import { app } from './lib/state.svelte';

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
      ArrowDown: () => app.move(1),
      ArrowUp: () => app.move(-1),
      s: () => void app.toggleStar(),
      m: () => void app.toggleRead(),
      o: () => app.openInBrowser(),
      r: () => void app.startRefresh(true),
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
  <div class="shell">
    <Sidebar />
    <ArticleList />
    <Reader />
  </div>
{/if}

<Toasts />

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--sidebar-width) var(--article-list-width) minmax(0, 1fr);
    height: 100%;
    overflow: hidden;
  }
</style>
