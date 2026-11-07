<script lang="ts">
  import type { Snippet } from 'svelte';
  import Gossamer from './Gossamer.svelte';

  let { title, children }: { title: string; children?: Snippet } = $props();
</script>

<div class="empty">
  <Gossamer />
  <h2>{title}</h2>
  {#if children}<p>{@render children()}</p>{/if}
</div>

<style>
  .empty {
    display: grid;
    justify-items: center;
    align-content: center;
    gap: var(--space-stack-sm);
    height: 100%;
    padding: var(--space-inset-xl);
    text-align: center;
  }
  h2 {
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-md);
    font-weight: var(--font-weight-strong);
    line-height: var(--font-line-height-heading);
  }
  p {
    max-width: 32ch;
    color: var(--color-text-muted);
  }
</style>
