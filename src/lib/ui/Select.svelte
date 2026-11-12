<script lang="ts">
  // A native select (its popup follows the theme via color-scheme) with the closed box drawn by
  // us: WebKitGTK otherwise paints it as a GTK combo box that ignores the app's styles.
  import Icon from '../components/Icon.svelte';

  let {
    value,
    options,
    label,
    onchange,
  }: {
    value: string;
    options: { value: string; label: string }[];
    /** Accessible name. */
    label: string;
    onchange: (value: string) => void;
  } = $props();
</script>

<span class="select-box">
  <select
    class="select"
    {value}
    aria-label={label}
    onchange={(e) => onchange((e.currentTarget as HTMLSelectElement).value)}
  >
    {#each options as option (option.value)}
      <option value={option.value}>{option.label}</option>
    {/each}
  </select>
  <span class="chevron" aria-hidden="true"><Icon name="chevron" size={14} /></span>
</span>

<style>
  .select-box {
    position: relative;
    display: inline-flex;
    width: 100%;
  }
  .select {
    appearance: none;
    -webkit-appearance: none;
    width: 100%;
    height: var(--input-height);
    padding-inline: var(--input-padding-inline) calc(var(--input-padding-inline) * 2 + 14px);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    color: var(--color-text-default);
    font: inherit;
    cursor: pointer;
    transition: border-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .select:hover {
    border-color: var(--color-border-strong);
  }
  .select:focus-visible {
    outline: var(--focus-ring-width) solid var(--color-border-focus);
    outline-offset: var(--focus-ring-offset);
  }
  option {
    background: var(--color-surface-overlay);
    color: var(--color-text-default);
  }
  .chevron {
    position: absolute;
    inset-block: 0;
    inset-inline-end: var(--input-padding-inline);
    display: grid;
    place-items: center;
    color: var(--color-text-muted);
    pointer-events: none;
    rotate: 90deg;
  }
</style>
