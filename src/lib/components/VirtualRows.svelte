<script lang="ts">
  // The virtualization primitive every Layout builds on: fixed-height rows, each holding
  // `perRow` Articles. Only rows near the viewport are rendered, so cost doesn't grow with the
  // number of Articles.
  import type { Snippet } from 'svelte';
  import { app } from '../state.svelte';

  let {
    rowHeight,
    perRow = 1,
    padding = 0,
    width = $bindable(0),
    label,
    row,
  }: {
    rowHeight: number;
    perRow?: number;
    padding?: number;
    width?: number;
    label: string;
    row: Snippet<[number]>;
  } = $props();

  const OVERSCAN_PX = 800;
  let viewport = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let height = $state(0);

  const rowCount = $derived(Math.ceil(app.total / Math.max(perRow, 1)));
  const safeHeight = $derived(Math.max(rowHeight, 1));
  const first = $derived(Math.max(0, Math.floor((scrollTop - padding - OVERSCAN_PX) / safeHeight)));
  const last = $derived(
    Math.min(rowCount, Math.ceil((scrollTop - padding + height + OVERSCAN_PX) / safeHeight)),
  );
  const visible = $derived(Array.from({ length: Math.max(0, last - first) }, (_, i) => first + i));

  $effect(() => {
    void app.ensureRange(first * perRow, last * perRow);
  });

  // A new list starts at the top.
  $effect(() => {
    void app.listEpoch;
    if (viewport) viewport.scrollTop = 0;
  });

  // Keep the selected Article in view during keyboard navigation.
  $effect(() => {
    const index = app.selectedIndex;
    if (index === null || !viewport) return;
    const top = padding + Math.floor(index / perRow) * safeHeight;
    if (top < viewport.scrollTop) viewport.scrollTop = top - padding;
    else if (top + safeHeight > viewport.scrollTop + viewport.clientHeight)
      viewport.scrollTop = top + safeHeight - viewport.clientHeight + padding;
  });
</script>

<div
  class="viewport"
  bind:this={viewport}
  bind:clientHeight={height}
  bind:clientWidth={width}
  onscroll={(e) => (scrollTop = (e.currentTarget as HTMLDivElement).scrollTop)}
  role="listbox"
  aria-label={label}
  tabindex="-1"
>
  <div class="spacer" style:height="{rowCount * safeHeight + padding * 2}px">
    {#each visible as index (index)}
      <div
        class="vrow"
        style:height="{safeHeight}px"
        style:transform="translateY({padding + index * safeHeight}px)"
        style:padding-inline="{padding}px"
      >
        {@render row(index)}
      </div>
    {/each}
  </div>
</div>

<style>
  .viewport {
    height: 100%;
    overflow-y: auto;
    min-height: 0;
    contain: strict;
    outline: none;
  }
  .spacer {
    position: relative;
  }
  .vrow {
    position: absolute;
    inset-inline: 0;
    top: 0;
    contain: layout paint;
  }
</style>
