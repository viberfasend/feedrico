<script lang="ts">
  import { articleOptionLabel } from '../../aria';
  import { shortDate } from '../../format';
  import { app } from '../../state.svelte';
  import { tokens } from '../../tokens/runtime.svelte';
  import Icon from '../Icon.svelte';
  import VirtualRows from '../VirtualRows.svelte';

  let { label }: { label: string } = $props();
  const rowHeight = $derived(tokens.px('--article-list-row-height') || 76);
</script>

<VirtualRows {rowHeight} {label}>
  {#snippet row(index)}
    {@const article = app.rowAt(index)}
    <div
      class="row"
      class:unread={article && !article.isRead}
      class:selected={app.selectedIndex === index}
      role="option"
      aria-selected={app.selectedIndex === index}
      aria-setsize={app.total}
      aria-posinset={index + 1}
      aria-label={article ? articleOptionLabel(article) : undefined}
      tabindex="-1"
      onclick={() => app.selectIndex(index)}
      onkeydown={(e) => e.key === 'Enter' && app.selectIndex(index)}
    >
      {#if article}
        <div class="meta">
          <span class="dot" aria-hidden="true"></span>
          <span class="feed">{article.feedTitle}</span>
          {#if article.isStarred}<span class="star"><Icon name="star" size={12} filled label="Starred" /></span>{/if}
          <time>{shortDate(article.articleDate)}</time>
        </div>
        <div class="title">{article.title}</div>
        <div class="excerpt">{article.excerpt}</div>
      {:else}
        <div class="skeleton"></div>
      {/if}
    </div>
  {/snippet}
</VirtualRows>

<style>
  .row {
    height: 100%;
    padding: var(--article-list-row-padding-block) var(--article-list-row-padding-inline);
    border-block-end: 1px solid var(--color-border-subtle);
    display: grid;
    grid-template-rows: auto auto 1fr;
    gap: var(--space-stack-2xs);
    overflow: hidden;
    cursor: pointer;
  }
  .row:hover {
    background: var(--color-surface-hover);
  }
  .row.selected {
    background: var(--color-surface-selected);
    /* 1.4.11: the selected state needs a 3:1 indicator, not just a background change. */
    box-shadow: inset 3px 0 0 var(--color-border-focus);
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--space-inline-xs);
    min-width: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .dot {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: var(--radius-pill);
  }
  .unread .dot {
    background: var(--color-unread-indicator);
  }
  .feed {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .star {
    display: inline-flex;
    color: var(--color-text-accent);
  }
  time {
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  .title,
  .excerpt {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    color: var(--color-text-muted);
    font-weight: var(--font-weight-medium);
  }
  .unread .title {
    color: var(--color-text-default);
    font-weight: var(--font-weight-strong);
  }
  .excerpt {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  :global([data-density='compact']) .excerpt {
    display: none;
  }
  .skeleton {
    grid-row: 1 / -1;
    margin-block: var(--space-stack-xs);
    border-radius: var(--radius-control);
    background: var(--color-surface-hover);
  }
</style>
