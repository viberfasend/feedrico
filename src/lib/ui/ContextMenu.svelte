<script lang="ts" module>
  export type { MenuItem } from './Menu.svelte';
</script>

<script lang="ts">
  import { ContextMenu } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import type { MenuItem } from './Menu.svelte';

  let {
    items,
    class: klass = '',
    children,
  }: {
    items: MenuItem[];
    class?: string;
    /** The element the menu belongs to. */
    children: Snippet;
  } = $props();

  // Bits opens on right-click and long-press. Keep the keyboard path (ContextMenu key,
  // Shift+F10) by re-dispatching a contextmenu event at the focused element's box.
  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return;
    event.preventDefault();
    event.stopPropagation();
    const el = event.target as HTMLElement;
    const box = el.getBoundingClientRect();
    el.dispatchEvent(
      new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: box.left + 24,
        clientY: box.bottom,
      }),
    );
  }
</script>

<ContextMenu.Root>
  <ContextMenu.Trigger>
    {#snippet child({ props })}
      <div {...props} class="context-trigger {klass}" onkeydown={onKeydown}>{@render children()}</div>
    {/snippet}
  </ContextMenu.Trigger>
  <ContextMenu.Portal>
    <ContextMenu.Content sideOffset={2}>
      {#snippet child({ props, wrapperProps })}
        <!-- bits-ui positions the wrapper; without it the menu lands at the page's end. -->
        <div {...wrapperProps}>
        <div {...props} class="menu">
          {#each items as item (item.label)}
            {#if item.separatorBefore}<div class="menu-separator" role="separator"></div>{/if}
            <ContextMenu.Item
              disabled={item.disabled}
              textValue={item.label}
              onSelect={() => item.onclick()}
            >
              {#snippet child({ props: itemProps })}
                <div {...itemProps} class="menu-item" class:danger={item.danger}>{item.label}</div>
              {/snippet}
            </ContextMenu.Item>
          {/each}
        </div>
        </div>
      {/snippet}
    </ContextMenu.Content>
  </ContextMenu.Portal>
</ContextMenu.Root>

<style>
  /* The trigger wraps the real row element without changing the layout. */
  .context-trigger {
    display: contents;
  }
  .menu {
    min-width: var(--menu-min-width);
    padding: var(--menu-padding);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    box-shadow: var(--shadow-overlay);
  }
  .menu-item {
    display: flex;
    align-items: center;
    height: var(--menu-item-height);
    padding-inline: var(--space-inset-md);
    border-radius: var(--radius-control);
    color: var(--color-text-default);
    cursor: pointer;
    outline: none;
  }
  .menu-item[data-highlighted] {
    background: var(--color-surface-hover);
  }
  .menu-item.danger {
    color: var(--color-status-warning-text);
  }
  .menu-separator {
    margin: var(--space-inset-xs) 0;
    border-top: 1px solid var(--color-border-subtle);
  }
</style>
