<script lang="ts">
  // The reader's own toolbar: stays at the top while the Article scrolls, so Star, Read and
  // Open in browser are always one click away, and every tooltip teaches the shortcut.
  import { hostOf } from '../../format';
  import { app } from '../../state.svelte';
  import IconButton from '../../ui/IconButton.svelte';
  import type { ArticleDetail } from '../../api';

  let { article, title, scrolled }: { article: ArticleDetail; title: string; scrolled: boolean } = $props();

  const index = $derived(app.selectedIndex ?? 0);
</script>

<div class="toolbar" class:scrolled role="toolbar" aria-label="Article">
  <div class="group">
    {#if app.isGrid}
      <IconButton label="Close reader" title="Close (Esc)" icon="close" onclick={() => app.closeReader()} />
    {/if}
    <IconButton label="Previous Article" title="Previous Article (k)" icon="up" disabled={index <= 0} onclick={() => void app.move(-1)} />
    <IconButton label="Next Article" title="Next Article (j)" icon="down" disabled={index >= app.total - 1} onclick={() => void app.move(1)} />
  </div>

  <!-- Once the heading scrolls away, the toolbar says which Article this is. -->
  <p class="title" aria-hidden="true">{title}</p>

  <div class="group">
    <IconButton
      label="Star"
      title={article.isStarred ? 'Unstar (s)' : 'Star (s)'}
      icon="star"
      pressed={article.isStarred}
      filled={article.isStarred}
      onclick={() => void app.toggleStar()}
    />
    <IconButton
      label={article.isRead ? 'Mark unread' : 'Mark read'}
      title={article.isRead ? 'Mark unread (m)' : 'Mark read (m)'}
      icon={article.isRead ? 'circle' : 'check'}
      onclick={() => void app.toggleRead()}
    />
    {#if article.url}
      <IconButton
        label={`Open on ${hostOf(article.url) || 'the web'}`}
        title={`Open on ${hostOf(article.url) || 'the web'} (o)`}
        icon="external"
        onclick={() => app.openInBrowser()}
      />
    {/if}
  </div>
</div>

<style>
  .toolbar {
    position: sticky;
    inset-block-start: 0;
    z-index: var(--layer-sticky);
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-inline-sm);
    padding: var(--space-inset-xs) var(--space-inset-md);
    background: var(--color-surface-raised);
    border-block-end: 1px solid transparent;
    transition: border-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .toolbar.scrolled {
    border-block-end-color: var(--color-border-subtle);
  }
  .group {
    display: flex;
    align-items: center;
    gap: var(--space-inline-xs);
  }
  .title {
    margin: 0;
    overflow: hidden;
    color: var(--color-text-default);
    font-size: var(--font-size-small);
    font-weight: var(--font-weight-medium);
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0;
    transition: opacity var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .scrolled .title {
    opacity: 1;
  }
  /* The Article's own toolbar is a query container (the reader pane), see Reader.svelte. */
  @container (max-width: 22rem) {
    .title {
      visibility: hidden;
    }
  }
</style>
