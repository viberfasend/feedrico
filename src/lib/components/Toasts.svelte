<script lang="ts">
  import { app } from '../state.svelte';
  import Toast from '../ui/Toast.svelte';
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each app.toasts as toast (toast.id)}
    <Toast
      message={toast.message}
      action={toast.action}
      onDismiss={() => app.dismiss(toast.id)}
    />
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    inset-block-end: var(--space-inset-xl);
    inset-inline-start: 50%;
    translate: -50% 0;
    display: grid;
    gap: var(--space-stack-sm);
    z-index: var(--layer-toast);
  }
</style>
