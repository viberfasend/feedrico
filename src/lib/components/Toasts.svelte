<script lang="ts">
  import { app } from '../state.svelte';
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each app.toasts as toast (toast.id)}
    <div class="toast">
      <span>{toast.message}</span>
      {#if toast.action}
        {@const action = toast.action}
        <button
          onclick={() => {
            action.run();
            app.dismiss(toast.id);
          }}>{action.label}</button
        >
      {/if}
    </div>
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
    z-index: 10;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: var(--space-inline-lg);
    padding: var(--space-inset-sm) var(--space-inset-lg);
    border-radius: var(--radius-pill);
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border-default);
    box-shadow: var(--shadow-overlay);
    animation: rise var(--motion-duration-base) var(--motion-easing-enter);
  }
  button {
    border: 0;
    background: none;
    color: var(--color-text-accent);
    font-weight: var(--font-weight-strong);
    cursor: pointer;
  }
  @keyframes rise {
    from {
      opacity: 0;
      translate: 0 8px;
    }
  }
</style>
