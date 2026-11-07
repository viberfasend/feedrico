<script lang="ts">
  import { mediaUrl } from '../../api';
  import { failedImages, hasLeadImage } from '../../images.svelte';
  import { app } from '../../state.svelte';
  import { tokens } from '../../tokens/runtime.svelte';
  import VirtualRows from '../VirtualRows.svelte';

  let { label }: { label: string } = $props();
  let width = $state(0);

  // Square tiles; the list itself only contains Articles with a Lead Image.
  const padding = $derived(tokens.px('--grid-padding'));
  const gap = $derived(tokens.px('--gallery-gap'));
  const minTile = $derived(tokens.px('--gallery-min-tile') || 180);
  const inner = $derived(Math.max(0, width - padding * 2));
  const columns = $derived(Math.max(1, Math.floor((inner + gap) / (minTile + gap))));
  const tile = $derived((inner - gap * (columns - 1)) / columns);

  $effect(() => {
    app.gridColumns = columns;
  });
</script>

<VirtualRows rowHeight={tile + gap} perRow={columns} {padding} bind:width {label}>
  {#snippet row(rowIndex)}
    {@const __ = failedImages.size}
    <div class="grid" style:grid-template-columns="repeat({columns}, 1fr)" style:gap="{gap}px">
      {#each Array.from({ length: columns }, (_, c) => rowIndex * columns + c) as index (index)}
        {@const article = index < app.total ? app.rowAt(index) : undefined}
        {#if article}
          <button
            class="tile"
            class:unread={!article.isRead}
            class:selected={app.selectedIndex === index}
            style:height="{tile}px"
            role="option"
            aria-selected={app.selectedIndex === index}
            aria-label={article.title}
            onclick={() => app.selectIndex(index)}
          >
            {#if hasLeadImage(article.leadImage)}
              <img
                src={mediaUrl(article.leadImage)}
                alt=""
                decoding="async"
                onerror={() => failedImages.add(article.leadImage!)}
              />
            {:else}
              <span class="fallback">{article.title}</span>
            {/if}
            <span class="caption">{article.title}</span>
          </button>
        {:else if index < app.total}
          <div class="tile skeleton" style:height="{tile}px"></div>
        {/if}
      {/each}
    </div>
  {/snippet}
</VirtualRows>

<style>
  .grid {
    display: grid;
  }
  .tile {
    position: relative;
    overflow: hidden;
    padding: 0;
    border: 0;
    border-radius: var(--radius-control);
    background: var(--color-surface-sunken);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: scale var(--motion-duration-slow) var(--motion-easing-standard);
  }
  .tile:hover img {
    scale: 1.03;
  }
  .tile:not(.unread) img {
    opacity: 0.72;
  }
  .tile.selected {
    outline: 2px solid var(--color-border-focus);
    outline-offset: 2px;
  }
  .unread::after {
    content: '';
    position: absolute;
    inset-block-start: var(--space-inset-sm);
    inset-inline-start: var(--space-inset-sm);
    width: 8px;
    height: 8px;
    border-radius: var(--radius-pill);
    background: var(--color-unread-indicator);
    box-shadow: 0 0 0 2px var(--color-surface-raised);
  }
  .caption {
    position: absolute;
    inset-inline: 0;
    inset-block-end: 0;
    padding: var(--space-inset-lg) var(--space-inset-sm) var(--space-inset-sm);
    background: linear-gradient(transparent, var(--color-media-scrim));
    color: var(--color-media-caption);
    font-size: var(--font-size-small);
    text-align: start;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0;
    transition: opacity var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .tile:hover .caption,
  .tile.selected .caption,
  .tile:focus-visible .caption {
    opacity: 1;
  }
  .fallback {
    display: grid;
    place-items: center;
    height: 100%;
    padding: var(--space-inset-md);
    color: var(--color-text-muted);
    font-family: var(--font-family-reading);
    font-size: var(--font-size-small);
  }
  .skeleton {
    background: var(--color-surface-hover);
  }
</style>
