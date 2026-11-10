<script lang="ts">
  import { Tooltip } from 'bits-ui';
  import type { Snippet } from 'svelte';

  let {
    text,
    side = 'top',
    children,
  }: {
    text: string;
    side?: 'top' | 'right' | 'bottom' | 'left';
    /** The wrapped trigger content. */
    children: Snippet;
  } = $props();
</script>

<Tooltip.Provider delayDuration={400}>
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <button {...props} type="button" class="tooltip-trigger">{@render children()}</button>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Portal>
      <Tooltip.Content {side} sideOffset={4}>
        {#snippet child({ props, wrapperProps })}
          <div {...wrapperProps}><div {...props} class="tooltip">{text}</div></div>
        {/snippet}
      </Tooltip.Content>
    </Tooltip.Portal>
  </Tooltip.Root>
</Tooltip.Provider>

<style>
  .tooltip-trigger {
    font: inherit;
    color: inherit;
  }
  .tooltip {
    padding: var(--tooltip-padding-block) var(--tooltip-padding-inline);
    border-radius: var(--radius-control);
    background: var(--color-surface-overlay);
    color: var(--color-text-default);
    font-size: var(--font-size-caption);
    box-shadow: var(--shadow-overlay);
  }
</style>
