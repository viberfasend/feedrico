<script lang="ts">
  import { Toggle as BitsToggle } from 'bits-ui';
  import type { Snippet } from 'svelte';

  let {
    pressed = $bindable(false),
    label,
    disabled = false,
    onchange,
    class: klass = '',
    children,
  }: {
    pressed?: boolean;
    /** Accessible name; also the text content when no children are given. */
    label: string;
    disabled?: boolean;
    onchange?: (pressed: boolean) => void;
    class?: string;
    children?: Snippet;
  } = $props();
</script>

<BitsToggle.Root bind:pressed {disabled} onPressedChange={onchange} aria-label={label}>
  {#snippet child({ props })}
    <button {...props} type="button" class="toggle {klass}">
      {#if children}{@render children()}{:else}{label}{/if}
    </button>
  {/snippet}
</BitsToggle.Root>

<style>
  .toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--button-gap);
    height: var(--button-height);
    padding-inline: var(--button-padding-inline);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    color: var(--color-text-default);
    cursor: pointer;
    transition: background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .toggle:hover:not(:disabled) {
    background: var(--color-surface-hover);
  }
  .toggle[data-state='on'] {
    border-color: transparent;
    background: var(--color-accent-default);
    color: var(--color-text-on-accent);
    font-weight: var(--font-weight-strong);
  }
  .toggle:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
