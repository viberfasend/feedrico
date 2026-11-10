<script lang="ts">
  import { Tooltip } from 'bits-ui';
  import Icon from '../components/Icon.svelte';
  import type { IconName } from '../components/Icon.svelte';

  let {
    label,
    icon,
    onclick,
    disabled = false,
    title,
    class: klass = '',
  }: {
    /** Required: the accessible name of an icon-only button. */
    label: string;
    icon: IconName;
    onclick?: (event: MouseEvent) => void;
    disabled?: boolean;
    /** Longer tooltip text; falls back to `label`. */
    title?: string;
    class?: string;
  } = $props();
</script>

<Tooltip.Provider delayDuration={400}>
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <button {...props} type="button" class="icon-button {klass}" aria-label={label} {disabled} {onclick}>
          <Icon name={icon} size={16} />
        </button>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Portal>
      <Tooltip.Content sideOffset={4}>
        {#snippet child({ props, wrapperProps })}
          <div {...wrapperProps}><div {...props} class="tooltip">{title ?? label}</div></div>
        {/snippet}
      </Tooltip.Content>
    </Tooltip.Portal>
  </Tooltip.Root>
</Tooltip.Provider>

<style>
  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--icon-button-size);
    height: var(--icon-button-size);
    border: 0;
    border-radius: var(--radius-control);
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background-color var(--motion-duration-fast) var(--motion-easing-standard),
      color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .icon-button:hover:not(:disabled) {
    background: var(--color-surface-hover);
    color: var(--color-text-default);
  }
  .icon-button:disabled {
    opacity: 0.6;
    cursor: default;
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
