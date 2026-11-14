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
    /** Makes this a radio item (`menuitemradio`); consecutive radio items form one group. */
    checked?: boolean;
  }
</script>

<script lang="ts">
  import { DropdownMenu } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import Icon from '../components/Icon.svelte';

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

  /** Consecutive radio items share one RadioGroup, started again by a heading or separator. */
  const rows = $derived.by(() => {
    const out: { first: MenuItem; items: MenuItem[]; radio: boolean }[] = [];
    for (const item of items) {
      const radio = item.checked !== undefined;
      const last = out[out.length - 1];
      if (last && last.radio === radio && !item.headingBefore && !item.separatorBefore) {
        last.items.push(item);
      } else {
        out.push({ first: item, items: [item], radio });
      }
    }
    return out;
  });
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
          {#each rows as row (row.first.label)}
            {#if row.first.separatorBefore}<div class="menu-separator" role="separator"></div>{/if}
            {#if row.first.headingBefore}<div class="menu-heading" aria-hidden="true">{row.first.headingBefore}</div>{/if}
            {#if row.radio}
              <DropdownMenu.RadioGroup value={row.items.find((i) => i.checked)?.label ?? ''}>
                {#snippet child({ props: groupProps })}
                  <div {...groupProps} aria-label={row.first.headingBefore}>
                    {#each row.items as item (item.label)}
                      <DropdownMenu.RadioItem
                        value={item.label}
                        disabled={item.disabled}
                        textValue={item.label}
                        onSelect={() => item.onclick()}
                      >
                        {#snippet child({ props: itemProps })}
                          <div {...itemProps} class="menu-item">
                            <!-- 1.4.1: the chosen item needs more than bold text to stand out. -->
                            <span class="radio-mark" aria-hidden="true">{#if item.checked}<Icon name="check" size={14} />{/if}</span>
                            {item.label}
                          </div>
                        {/snippet}
                      </DropdownMenu.RadioItem>
                    {/each}
                  </div>
                {/snippet}
              </DropdownMenu.RadioGroup>
            {:else}
              {#each row.items as item (item.label)}
                <DropdownMenu.Item
                  disabled={item.disabled}
                  textValue={item.label}
                  onSelect={() => item.onclick()}
                >
                  {#snippet child({ props: itemProps })}
                    <div {...itemProps} class="menu-item" class:danger={item.danger}>
                      {item.label}
                    </div>
                  {/snippet}
                </DropdownMenu.Item>
              {/each}
            {/if}
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
    /* bits-ui copies this onto the positioned wrapper. */
    z-index: var(--layer-popover);
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
  .menu-item[aria-checked='true'] {
    font-weight: var(--font-weight-strong);
  }
  .radio-mark {
    display: inline-flex;
    flex: none;
    width: 14px;
    margin-inline-end: var(--space-inline-sm);
    color: var(--color-text-accent);
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
