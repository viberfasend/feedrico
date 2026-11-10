<script lang="ts">
  let {
    message,
    action,
    onDismiss,
  }: {
    message: string;
    action?: { label: string; run: () => void };
    onDismiss?: () => void;
  } = $props();
</script>

<div class="toast">
  <span>{message}</span>
  {#if action}
    {@const a = action}
    <button type="button" onclick={() => { a.run(); onDismiss?.(); }}>{a.label}</button>
  {/if}
</div>

<style>
  .toast {
    display: flex;
    align-items: center;
    gap: var(--space-inline-lg);
    padding: var(--toast-padding-block) var(--toast-padding-inline);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-pill);
    background: var(--color-surface-overlay);
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
