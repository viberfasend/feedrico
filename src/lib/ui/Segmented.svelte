<script lang="ts" generics="T extends string">
  // A small set of exclusive choices shown side by side. Native radios underneath, so arrow
  // keys, focus and screen readers behave as a radio group without extra code.
  let {
    value,
    options,
    label,
    onchange,
  }: {
    value: T;
    options: { value: T; label: string; font?: string }[];
    /** Accessible name of the group. */
    label: string;
    onchange: (value: T) => void;
  } = $props();

  const name = $props.id();
</script>

<div class="segmented" role="radiogroup" aria-label={label}>
  {#each options as option (option.value)}
    <label class="segment" class:active={option.value === value}>
      <input
        type="radio"
        {name}
        value={option.value}
        checked={option.value === value}
        onchange={() => onchange(option.value)}
      />
      <span style:font-family={option.font}>{option.label}</span>
    </label>
  {/each}
</div>

<style>
  .segmented {
    display: inline-grid;
    grid-auto-flow: column;
    grid-auto-columns: 1fr;
    padding: var(--tabs-list-padding);
    border-radius: var(--radius-control);
    background: var(--color-surface-sunken);
    box-shadow: inset 0 0 0 1px var(--color-border-subtle);
  }
  .segment {
    position: relative;
    display: grid;
    place-items: center;
    min-height: calc(var(--input-height) - var(--tabs-list-padding) * 2);
    padding-inline: var(--space-inset-md);
    border-radius: calc(var(--radius-control) - 2px);
    color: var(--color-text-muted);
    cursor: pointer;
    white-space: nowrap;
    transition:
      background-color var(--motion-duration-fast) var(--motion-easing-standard),
      color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .segment:hover {
    color: var(--color-text-default);
  }
  .segment.active {
    background: var(--color-surface-raised);
    color: var(--color-text-default);
    font-weight: var(--font-weight-medium);
    box-shadow: var(--shadow-raised), inset 0 0 0 1px var(--color-border-subtle);
  }
  input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .segment:has(input:focus-visible) {
    outline: var(--focus-ring-width) solid var(--color-border-focus);
    outline-offset: var(--focus-ring-offset);
  }
</style>
