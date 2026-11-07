<script lang="ts">
  import { mediaUrl } from '../../api';
  import { shortDate } from '../../format';
  import { failedImages, hasLeadImage } from '../../images.svelte';
  import { app } from '../../state.svelte';
  import { tokens } from '../../tokens/runtime.svelte';
  import Gossamer from '../Gossamer.svelte';
  import Icon from '../Icon.svelte';
  import VirtualRows from '../VirtualRows.svelte';

  let { label }: { label: string } = $props();
  let width = $state(0);

  // Uniform grid: every card has the same height, so rows can be virtualized exactly.
  const padding = $derived(tokens.px('--grid-padding'));
  const gap = $derived(tokens.px('--cards-gap'));
  const minWidth = $derived(tokens.px('--cards-min-width') || 260);
  const inner = $derived(Math.max(0, width - padding * 2));
  const columns = $derived(Math.max(1, Math.floor((inner + gap) / (minWidth + gap))));
  const cardWidth = $derived((inner - gap * (columns - 1)) / columns);
  const imageHeight = $derived(Math.round(cardWidth * (tokens.px('--cards-image-ratio') || 0.6667)));
  const cardHeight = $derived(imageHeight + tokens.px('--cards-text-height'));

  $effect(() => {
    app.gridColumns = columns;
  });
</script>

<VirtualRows rowHeight={cardHeight + gap} perRow={columns} {padding} bind:width {label}>
  {#snippet row(rowIndex)}
    {@const _ = app.pagesVersion}
    <div class="grid" style:grid-template-columns="repeat({columns}, 1fr)" style:gap="{gap}px">
      {#each Array.from({ length: columns }, (_, c) => rowIndex * columns + c) as index (index)}
        {@const article = index < app.total ? app.rowAt(index) : undefined}
        {#if article}
          {@const withImage = hasLeadImage(article.leadImage)}
          <button
            class="card"
            class:unread={!article.isRead}
            class:selected={app.selectedIndex === index}
            style:height="{cardHeight}px"
            role="option"
            aria-selected={app.selectedIndex === index}
            onclick={() => app.selectIndex(index)}
          >
            <div class="visual" style:height="{imageHeight}px">
              {#if withImage}
                <img
                  src={mediaUrl(article.leadImage!)}
                  alt=""
                  loading="lazy"
                  decoding="async"
                  onerror={() => failedImages.add(article.leadImage!)}
                />
              {:else}
                <div class="typographic">
                  <span class="motif"><Gossamer size={120} /></span>
                  <span class="big-title">{article.title}</span>
                </div>
              {/if}
            </div>
            <div class="body">
              <div class="meta">
                <span class="dot" aria-hidden="true"></span>
                <span class="feed">{article.feedTitle}</span>
                {#if article.isStarred}<span class="star"><Icon name="star" size={12} filled label="Starred" /></span>{/if}
                <time>{shortDate(article.articleDate)}</time>
              </div>
              {#if withImage}
                <div class="title">{article.title}</div>
              {/if}
              <div class="excerpt" class:long={!withImage}>{article.excerpt}</div>
            </div>
          </button>
        {:else if index < app.total}
          <div class="card skeleton" style:height="{cardHeight}px"></div>
        {/if}
      {/each}
    </div>
  {/snippet}
</VirtualRows>

<style>
  .grid {
    display: grid;
  }
  .card {
    display: grid;
    grid-template-rows: auto 1fr;
    overflow: hidden;
    padding: 0;
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-card);
    background: var(--color-surface-raised);
    color: inherit;
    text-align: start;
    font: inherit;
    cursor: pointer;
    transition:
      box-shadow var(--motion-duration-fast) var(--motion-easing-standard),
      border-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .card:hover {
    box-shadow: var(--shadow-raised);
    border-color: var(--color-border-default);
  }
  .card.selected {
    border-color: var(--color-border-focus);
    box-shadow: 0 0 0 1px var(--color-border-focus);
  }
  .visual {
    overflow: hidden;
    background: var(--color-surface-sunken);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .typographic {
    position: relative;
    height: 100%;
    display: flex;
    align-items: flex-end;
    padding: var(--space-inset-lg);
    background: linear-gradient(160deg, var(--color-accent-subtle), var(--color-surface-sunken));
  }
  .motif {
    position: absolute;
    inset-block-start: var(--space-inset-sm);
    inset-inline-end: var(--space-inset-sm);
    opacity: 0.8;
  }
  .big-title {
    position: relative;
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-md);
    font-weight: var(--font-weight-heading);
    line-height: var(--font-line-height-heading);
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .body {
    display: grid;
    align-content: start;
    gap: var(--space-stack-xs);
    padding: var(--space-inset-md);
    min-height: 0;
    overflow: hidden;
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
  }
  .title {
    font-family: var(--font-family-reading);
    font-weight: var(--font-weight-strong);
    line-height: var(--font-line-height-heading);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .card:not(.unread) .title,
  .card:not(.unread) .big-title {
    color: var(--color-text-muted);
  }
  .excerpt {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .excerpt.long {
    -webkit-line-clamp: 4;
    line-clamp: 4;
  }
  .skeleton {
    background: var(--color-surface-hover);
    border-color: transparent;
  }
</style>
