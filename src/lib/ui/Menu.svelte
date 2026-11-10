<script lang="ts" module>
  /** One entry of a Menu or ContextMenu. */
  export interface MenuItem {
    label: string;
    onclick: () => void;
    danger?: boolean;
    disabled?: boolean;
    /** Renders a separator above this item (e.g. before a destructive action). */
    separatorBefore?: boolean;
    /** Renders a non-interactive heading above this item. */
    headingBefore?: string;
  }
</script>

<script lang="ts">
  import { DropdownMenu } from 'bits-ui';
  import type { Snippet } from 'svelte';

  let {
    items,
    label,
    open = $bindable(false),
    align = 'start',
    class: klass = '',
    children,
  }: {
    items: MenuItem[];
    /** Accessible name for the trigger; required when the trigger shows only an icon. */
    label?: string;
    open?: boolean;
    align?: 'start' | 'center' | 'end';
    class?: string;
    /** The trigger content. */
    children: Snippet;
  } = $props();
</script>

<DropdownMenu.Root bind:open>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}
      <button {...props} type="button" aria-label={label} title={label} class="menu-trigger {klass}">{@render children()}</button>
    {/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Portal>
    <DropdownMenu.Content {align} sideOffset={4}>
      {#snippet child({ props, wrapperProps })}
        <!-- bits-ui positions the wrapper; without it the menu lands at the page's end. -->
        <div {...wrapperProps}>
        <div {...props} class="menu">
          {#each items as item (item.label)}
            {#if item.separatorBefore}<div class="menu-separator" role="separator"></div>{/if}
            {#if item.headingBefore}<div class="menu-heading">{item.headingBefore}</div>{/if}
            <DropdownMenu.Item
              disabled={item.disabled}
              textValue={item.label}
              onSelect={() => item.onclick()}
            >
              {#snippet child({ props: itemProps })}
                <div {...itemProps} class="menu-item" class:danger={item.danger}>{item.label}</div>
              {/snippet}
            </DropdownMenu.Item>
          {/each}
        </div>
        </div>
      {/snippet}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>

<style>
  .menu-trigger {
    font: inherit;
    color: inherit;
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
  .menu-heading {
    padding: var(--space-inset-sm) var(--space-inset-md) 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
</style>
