<script lang="ts">
  // A Feed's favicon from the media cache, or a generated tile: the first letter of the Feed
  // title on a colour picked by Feed id, so every source has the same visual weight.
  import { mediaUrl } from '../api';
  import { failedImages } from '../images.svelte';
  import { feedInitial, feedTile } from './feed-icon';

  let { feedId, title, favicon }: { feedId: number; title: string; favicon: string | null } = $props();

  const showImage = $derived(!!favicon && !failedImages.has(favicon));
</script>

{#if showImage}
  <img
    class="icon"
    src={mediaUrl(favicon!)}
    alt=""
    decoding="async"
    onerror={() => failedImages.add(favicon!)}
  />
{:else}
  <span class="icon tile" style:background="var(--color-feed-tile-{feedTile(feedId)})" aria-hidden="true"
    >{feedInitial(title)}</span
  >
{/if}

<style>
  .icon {
    flex: none;
    display: inline-block;
    width: var(--feed-icon-size);
    height: var(--feed-icon-size);
    border-radius: var(--feed-icon-radius);
    object-fit: contain;
  }
  .tile {
    display: inline-grid;
    place-items: center;
    color: var(--color-feed-tile-text);
    font-family: var(--font-family-ui);
    font-size: calc(var(--feed-icon-size) * 0.625);
    font-weight: var(--font-weight-heading);
    line-height: 1;
    user-select: none;
  }
</style>
