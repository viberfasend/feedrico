<script lang="ts">
  import { app } from '../state.svelte';
  import Gossamer from './Gossamer.svelte';

  const attention = $derived(app.attention);
  const status = $derived(attention?.status);
  const when = $derived(
    status?.kind === 'locked' && status.openedAt
      ? new Date(status.openedAt * 1000).toLocaleString()
      : 'an unknown time',
  );
</script>

{#if attention && status}
  <main class="attention">
    <div class="card" role="alertdialog" aria-labelledby="attention-title">
      <Gossamer size={160} />
      {#if status.kind === 'locked'}
        <h1 id="attention-title">This Library is still marked as open</h1>
        <p>
          It was last open on <strong>{status.machine}</strong> at {when} and wasn't closed cleanly.
          If it isn't open anywhere else right now, it's safe to continue.
        </p>
        <div class="actions">
          <button class="primary" onclick={() => app.open(attention.path, false, true)}>
            Open anyway
          </button>
          <button onclick={() => app.showWelcome()}>Choose another Library</button>
        </div>
      {:else if status.kind === 'needsUpgrade'}
        <h1 id="attention-title">Upgrade this Library?</h1>
        <p>
          It was made by an older Feedrico. After upgrading, older versions of Feedrico won't open it
          anymore. A backup of the database is kept inside the Library.
        </p>
        <div class="actions">
          <button class="primary" onclick={() => app.open(attention.path, true, false)}>
            Upgrade and open
          </button>
          <button onclick={() => app.showWelcome()}>Not now</button>
        </div>
      {:else if status.kind === 'tooNew'}
        <h1 id="attention-title">This Library needs a newer Feedrico</h1>
        <p>
          It was written by a newer version of Feedrico (format {status.found}; this one supports up
          to {status.supported}). Nothing was changed. Update Feedrico to open it.
        </p>
        <div class="actions">
          <button onclick={() => app.showWelcome()}>Choose another Library</button>
        </div>
      {:else}
        <h1 id="attention-title">This folder isn't a Feedrico Library</h1>
        <div class="actions">
          <button onclick={() => app.showWelcome()}>Back</button>
        </div>
      {/if}
      <p class="path">{attention.path}</p>
    </div>
  </main>
{/if}

<style>
  .attention {
    display: grid;
    place-items: center;
    height: 100%;
    padding: var(--space-inset-2xl);
  }
  .card {
    display: grid;
    justify-items: center;
    gap: var(--space-stack-md);
    max-width: 32rem;
    text-align: center;
  }
  h1 {
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-lg);
    line-height: var(--font-line-height-heading);
  }
  p {
    color: var(--color-text-muted);
    line-height: var(--font-line-height-body);
  }
  strong {
    color: var(--color-text-default);
  }
  .actions {
    display: flex;
    gap: var(--space-inline-sm);
  }
  button {
    height: var(--control-height);
    padding-inline: var(--space-inset-lg);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    cursor: pointer;
  }
  .primary {
    border-color: transparent;
    background: var(--color-accent-default);
    color: var(--color-text-on-accent);
    font-weight: var(--font-weight-strong);
  }
  .path {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-caption);
  }
</style>
