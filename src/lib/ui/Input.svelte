<script lang="ts">
  let {
    value = $bindable(''),
    type = 'text',
    placeholder = '',
    id,
    disabled = false,
    class: klass = '',
    ...rest
  }: {
    value?: string;
    type?: string;
    placeholder?: string;
    id?: string;
    disabled?: boolean;
    class?: string;
    [key: string]: unknown;
  } = $props();
</script>

<input bind:value {type} {placeholder} {id} {disabled} class="input {klass}" {...rest} />

<style>
  .input {
    height: var(--input-height);
    padding-inline: var(--input-padding-inline);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-base);
    color: var(--color-text-default);
    font: inherit;
  }
  .input:focus-visible {
    border-color: var(--color-border-focus);
  }
  .input:disabled {
    opacity: 0.6;
  }
</style>
