<script lang="ts">
  import { Tabs as BitsTabs } from 'bits-ui';
  import type { Snippet } from 'svelte';

  let {
    value = $bindable(''),
    tabs,
    class: klass = '',
    children,
  }: {
    value?: string;
    tabs: { value: string; label: string }[];
    class?: string;
    /** Renders one panel; receives the tab's value. */
    children: Snippet<[string]>;
  } = $props();
</script>

<BitsTabs.Root bind:value>
  {#snippet child({ props })}
    <div {...props} class="tabs {klass}">
      <BitsTabs.List>
        {#snippet child({ props: listProps })}
          <div {...listProps} class="tabs-list">
            {#each tabs as tab (tab.value)}
              <BitsTabs.Trigger value={tab.value}>
                {#snippet child({ props: triggerProps })}
                  <button {...triggerProps} type="button" class="tabs-trigger">{tab.label}</button>
                {/snippet}
              </BitsTabs.Trigger>
            {/each}
          </div>
        {/snippet}
      </BitsTabs.List>
      {#each tabs as tab (tab.value)}
        <BitsTabs.Content value={tab.value}>
          {#snippet child({ props: contentProps })}
            <div {...contentProps} class="tabs-content">{@render children(tab.value)}</div>
          {/snippet}
        </BitsTabs.Content>
      {/each}
    </div>
  {/snippet}
</BitsTabs.Root>

<style>
  .tabs {
    display: grid;
    gap: var(--space-stack-sm);
  }
  .tabs-list {
    display: flex;
    gap: var(--space-inline-xs);
    padding: var(--tabs-list-padding);
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-control);
    background: var(--color-surface-sunken);
  }
  .tabs-trigger {
    height: var(--tabs-trigger-height);
    padding-inline: var(--space-inset-md);
    border: 0;
    border-radius: var(--radius-control);
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .tabs-trigger:hover {
    background: var(--color-surface-hover);
    color: var(--color-text-default);
  }
  .tabs-trigger[data-state='active'] {
    background: var(--color-surface-raised);
    color: var(--color-text-default);
    font-weight: var(--font-weight-medium);
  }
  .tabs-content {
    outline: none;
  }
</style>
