<script lang="ts">
  import { Toggle as BitsToggle } from 'bits-ui';
  import type { Snippet } from 'svelte';

  let {
    pressed = $bindable(false),
    label,
    disabled = false,
    onchange,
    variant = 'button',
    class: klass = '',
    children,
  }: {
    pressed?: boolean;
    /** Accessible name; also the text content when no children are given. */
    label: string;
    disabled?: boolean;
    onchange?: (pressed: boolean) => void;
    /** `switch`: an on/off track; the text is for screen readers only. */
    variant?: 'button' | 'switch';
    class?: string;
    children?: Snippet;
  } = $props();
</script>

<BitsToggle.Root bind:pressed {disabled} onPressedChange={onchange} aria-label={label}>
  {#snippet child({ props })}
    {#if variant === 'switch'}
      <button {...props} type="button" class="switch {klass}">
        <span class="sr-only">{#if children}{@render children()}{:else}{label}{/if}</span>
        <span class="thumb" aria-hidden="true"></span>
      </button>
    {:else}
      <button {...props} type="button" class="toggle {klass}">
        {#if children}{@render children()}{:else}{label}{/if}
      </button>
    {/if}
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
  .switch {
    position: relative;
    flex: none;
    width: var(--switch-width);
    height: var(--switch-height);
    padding: 0;
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-pill);
    background: var(--color-surface-sunken);
    cursor: pointer;
    transition:
      background-color var(--motion-duration-fast) var(--motion-easing-standard),
      border-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .thumb {
    position: absolute;
    inset-block-start: 1px;
    inset-inline-start: 1px;
    width: calc(var(--switch-height) - 4px);
    height: calc(var(--switch-height) - 4px);
    border-radius: var(--radius-pill);
    background: var(--color-text-muted);
    transition:
      translate var(--motion-duration-fast) var(--motion-easing-standard),
      background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .switch[data-state='on'] {
    border-color: transparent;
    background: var(--color-accent-default);
  }
  .switch[data-state='on'] .thumb {
    translate: calc(var(--switch-width) - var(--switch-height)) 0;
    background: var(--color-text-on-accent);
  }
  .switch:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
