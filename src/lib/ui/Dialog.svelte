<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Snippet } from 'svelte';

  let {
    open = $bindable(false),
    title,
    class: klass = '',
    children,
  }: {
    open?: boolean;
    title: string;
    class?: string;
    children: Snippet;
  } = $props();

  // Focus returns to whatever had it when the dialog opened.
  let previousFocus: HTMLElement | null = null;
  $effect(() => {
    if (open) {
      previousFocus = document.activeElement as HTMLElement | null;
    } else if (previousFocus) {
      previousFocus.focus({ preventScroll: true });
      previousFocus = null;
    }
  });
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay>
      {#snippet child({ props })}
        <div {...props} class="scrim"></div>
      {/snippet}
    </Dialog.Overlay>
    <Dialog.Content>
      {#snippet child({ props })}
        <div {...props} class="dialog {klass}">
          <Dialog.Title>
            {#snippet child({ props: titleProps })}
              <h2 {...titleProps} class="dialog-title">{title}</h2>
            {/snippet}
          </Dialog.Title>
          {@render children()}
        </div>
      {/snippet}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    background: color-mix(in srgb, black 40%, transparent);
  }
  .dialog {
    position: fixed;
    inset: 0;
    margin: auto;
    width: min(var(--dialog-width), calc(100vw - 2rem));
    height: fit-content;
    max-height: calc(100vh - 2rem);
    overflow-y: auto;
    padding: var(--dialog-padding);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    box-shadow: var(--shadow-overlay);
    outline: none;
  }
  .dialog-title {
    margin: 0 0 var(--space-stack-md);
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-md);
    font-weight: var(--font-weight-heading);
  }
</style>
