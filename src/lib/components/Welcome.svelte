<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { join } from '@tauri-apps/api/path';
  import { app } from '../state.svelte';
  import Gossamer from './Gossamer.svelte';
  import Icon from './Icon.svelte';

  let busy = $state(false);

  async function run(task: () => Promise<void>) {
    busy = true;
    try {
      await task();
    } finally {
      busy = false;
    }
  }

  const createDefault = () => run(() => app.create(app.defaultPath));

  const createElsewhere = () =>
    run(async () => {
      const dir = await open({ directory: true, title: 'Where should your Library live?' });
      if (typeof dir === 'string') await app.create(await join(dir, 'Feedrico Library'));
    });

  const openExisting = () =>
    run(async () => {
      const dir = await open({ directory: true, title: 'Open a Feedrico Library' });
      if (typeof dir === 'string') await app.open(dir);
    });
</script>

<main class="welcome">
  <div class="card">
    <Gossamer size={220} />
    <h1>Feedrico</h1>
    <p class="lede">
      Your Feeds, Articles and reading state live in one <strong>Library</strong> folder. Keep it
      here, or put it on a USB stick and take it anywhere.
    </p>

    <div class="actions">
      {#if app.defaultPath}
        <button class="primary" disabled={busy} onclick={createDefault}>
          <Icon name="library" /> Create new Library
        </button>
        <p class="path" title={app.defaultPath}>{app.defaultPath}</p>
      {/if}
      <div class="row">
        <button disabled={busy} onclick={createElsewhere}>Create in another place…</button>
        <button disabled={busy} onclick={openExisting}>Open existing Library…</button>
      </div>
    </div>

    {#if app.recent.length}
      <section class="recent">
        <h2>Recent Libraries</h2>
        <ul>
          {#each app.recent as path (path)}
            <li>
              <button class="link" disabled={busy} onclick={() => run(() => app.open(path))}>
                {path}
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  </div>
</main>

<style>
  .welcome {
    display: grid;
    place-items: center;
    height: 100%;
    padding: var(--space-inset-2xl);
    background:
      radial-gradient(
        ellipse at 70% 110%,
        var(--color-accent-subtle) 0%,
        transparent 60%
      ),
      var(--color-surface-base);
  }
  .card {
    display: grid;
    justify-items: center;
    gap: var(--space-stack-md);
    max-width: 34rem;
    text-align: center;
  }
  h1 {
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-display);
    font-weight: var(--font-weight-heading);
    color: var(--color-text-accent);
    letter-spacing: -0.02em;
    line-height: var(--font-line-height-heading);
  }
  .lede {
    color: var(--color-text-muted);
    font-size: var(--font-size-title-sm);
    line-height: var(--font-line-height-body);
  }
  .actions {
    display: grid;
    justify-items: center;
    gap: var(--space-stack-sm);
    margin-block-start: var(--space-stack-md);
  }
  .row {
    display: flex;
    gap: var(--space-inline-sm);
    flex-wrap: wrap;
    justify-content: center;
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-inline-sm);
    height: var(--control-height);
    padding-inline: var(--space-inset-lg);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    cursor: pointer;
    transition: background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  button:hover:not(:disabled) {
    background: var(--color-surface-hover);
  }
  button:disabled {
    opacity: 0.6;
    cursor: progress;
  }
  .primary {
    height: calc(var(--control-height) + var(--space-inset-sm));
    padding-inline: var(--space-inset-xl);
    border-color: transparent;
    background: var(--color-accent-default);
    color: var(--color-text-on-accent);
    font-weight: var(--font-weight-strong);
  }
  .primary:hover:not(:disabled) {
    background: var(--color-accent-hover);
  }
  .path {
    max-width: 30rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-caption);
  }
  .recent {
    margin-block-start: var(--space-stack-lg);
    width: 100%;
    text-align: start;
  }
  h2 {
    font-size: var(--font-size-small);
    font-weight: var(--font-weight-strong);
    color: var(--color-text-muted);
    margin-block-end: var(--space-stack-xs);
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .link {
    width: 100%;
    border: 0;
    background: none;
    justify-content: flex-start;
    padding-inline: var(--space-inset-sm);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-caption);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
