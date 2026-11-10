<script lang="ts">
  import { Button as BitsButton } from 'bits-ui';
  import type { Snippet } from 'svelte';

  let {
    variant = 'default',
    type = 'button',
    disabled = false,
    class: klass = '',
    onclick,
    children,
  }: {
    variant?: 'default' | 'primary' | 'danger' | 'ghost';
    type?: 'button' | 'submit' | 'reset';
    disabled?: boolean;
    class?: string;
    onclick?: (event: MouseEvent) => void;
    children: Snippet;
  } = $props();
</script>

<!-- bits' Button.Root renders the element itself, so the classes are styled globally. -->
<BitsButton.Root {type} {disabled} {onclick} class="btn variant-{variant} {klass}">
  {@render children()}
</BitsButton.Root>

<style>
  :global(.btn) {
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
  :global(.btn:hover:not(:disabled)) {
    background: var(--color-surface-hover);
  }
  :global(.btn:disabled) {
    opacity: 0.6;
    cursor: default;
  }
  :global(.variant-primary) {
    border-color: transparent;
    background: var(--color-accent-default);
    color: var(--color-text-on-accent);
    font-weight: var(--font-weight-strong);
  }
  :global(.variant-primary:hover:not(:disabled)) {
    background: var(--color-accent-hover);
  }
  :global(.variant-danger) {
    color: var(--color-status-danger-text);
  }
  :global(.variant-ghost) {
    border-color: transparent;
    background: none;
  }
  :global(.variant-ghost:hover:not(:disabled)) {
    background: var(--color-surface-hover);
  }
</style>
