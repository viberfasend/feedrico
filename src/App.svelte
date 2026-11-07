<script lang="ts">
  import EmptyState from './lib/components/EmptyState.svelte';
  import { applyDensity, applyTheme, type Density, type Theme } from './lib/tokens';

  // Placeholder shell for the three-pane layout (#20). Theme and density will come from the
  // Library's settings (#28); for now they can be toggled to exercise the token sets.
  const themes: Theme[] = ['system', 'light', 'dark'];
  let theme = $state<Theme>('system');
  let density = $state<Density>('comfortable');

  $effect(() => applyTheme(theme));
  $effect(() => applyDensity(density));

  const smartViews = ['All Unread', 'Starred', 'Today'] as const;
  let selected = $state<(typeof smartViews)[number]>('All Unread');
</script>

<div class="shell">
  <nav class="sidebar" aria-label="Feeds">
    <header class="brand">
      <span class="wordmark">Feedrico</span>
    </header>

    <ul class="views">
      {#each smartViews as view (view)}
        <li>
          <button
            class="row"
            aria-current={selected === view ? 'page' : undefined}
            onclick={() => (selected = view)}
          >
            {view}
          </button>
        </li>
      {/each}
    </ul>

    <div class="feeds">
      <p class="hint">No Feeds yet. Import an OPML file to get started.</p>
    </div>

    <footer class="toolbar">
      <button
        class="chip"
        onclick={() => (theme = themes[(themes.indexOf(theme) + 1) % themes.length] ?? 'system')}
      >
        Theme: {theme}
      </button>
      <button
        class="chip"
        onclick={() => (density = density === 'comfortable' ? 'compact' : 'comfortable')}
      >
        {density}
      </button>
    </footer>
  </nav>

  <section class="list" aria-label="Articles">
    <header class="pane-header">
      <h1>{selected}</h1>
    </header>
    <EmptyState title="Nothing here yet">Articles from your Feeds will drift in here.</EmptyState>
  </section>

  <main class="reader" aria-label="Reader">
    <EmptyState title="Pick an Article">Press <kbd>j</kbd> and <kbd>k</kbd> to move through the list.</EmptyState>
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--sidebar-width) var(--article-list-width) 1fr;
    height: 100%;
  }

  .sidebar {
    display: grid;
    grid-template-rows: auto auto 1fr auto;
    background: var(--color-surface-sunken);
    border-right: 1px solid var(--color-border-subtle);
    padding: var(--space-inset-sm);
    gap: var(--space-stack-md);
  }

  .brand {
    padding: var(--space-inset-sm) var(--sidebar-row-padding-inline);
  }

  .wordmark {
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-md);
    font-weight: var(--font-weight-heading);
    color: var(--color-text-accent);
    letter-spacing: -0.01em;
  }

  .views {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--space-stack-2xs);
  }

  .row {
    width: 100%;
    height: var(--sidebar-row-height);
    padding-inline: var(--sidebar-row-padding-inline);
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    text-align: start;
    cursor: pointer;
    transition: background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }

  .row:hover {
    background: var(--color-surface-hover);
  }

  .row[aria-current='page'] {
    background: var(--color-surface-selected);
    font-weight: var(--font-weight-medium);
  }

  .hint {
    padding-inline: var(--sidebar-row-padding-inline);
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }

  .toolbar {
    display: flex;
    gap: var(--space-inline-sm);
  }

  .chip {
    height: var(--control-height);
    padding-inline: var(--space-inset-md);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-pill);
    background: var(--color-surface-raised);
    font-size: var(--font-size-caption);
    cursor: pointer;
  }

  .list {
    display: grid;
    grid-template-rows: auto 1fr;
    background: var(--color-surface-base);
    border-right: 1px solid var(--color-border-subtle);
    min-width: 0;
  }

  .pane-header {
    padding: var(--space-inset-lg);
    border-bottom: 1px solid var(--color-border-subtle);
  }

  h1 {
    font-size: var(--font-size-title-sm);
    font-weight: var(--font-weight-strong);
  }

  .reader {
    background: var(--color-surface-raised);
    min-width: 0;
  }

  kbd {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-caption);
    padding: 0 var(--space-inset-xs);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-sunken);
  }
</style>
