<script lang="ts">
  import { api, type Discovery, type FeedCandidate } from '../api';
  import { app, errorMessage } from '../state.svelte';

  let input = $state('');
  let folderId = $state<number | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let choices = $state<FeedCandidate[] | null>(null);

  function close() {
    app.addFeedOpen = false;
  }

  async function subscribe(url: string, title: string, siteUrl: string | null) {
    if (await app.addFeed(url, title, siteUrl, folderId)) close();
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy || !input.trim()) return;
    busy = true;
    error = null;
    choices = null;
    try {
      const found: Discovery = await api.discoverFeed(input);
      if (found.kind === 'found') await subscribe(found.url, found.title, found.siteUrl);
      else choices = found.candidates;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  function focus(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="add-feed-title">
    <h2 id="add-feed-title">Add Feed</h2>
    <form onsubmit={submit}>
      <label for="add-feed-url">Feed or website address</label>
      <input
        id="add-feed-url"
        type="text"
        inputmode="url"
        autocomplete="off"
        spellcheck="false"
        placeholder="example.com or https://example.com/feed.xml"
        bind:value={input}
        use:focus
      />
      <label for="add-feed-folder">Folder</label>
      <select id="add-feed-folder" bind:value={folderId}>
        <option value={null}>No Folder (top level)</option>
        {#each app.sidebar?.folders ?? [] as folder (folder.id)}
          <option value={folder.id}>{folder.name}</option>
        {/each}
      </select>

      {#if error}<p class="error" role="alert">{error}</p>{/if}

      {#if choices}
        <fieldset>
          <legend>Several Feeds found. Pick one:</legend>
          {#each choices as choice (choice.url)}
            <button type="button" class="choice" onclick={() => subscribe(choice.url, choice.title ?? choice.url, null)}>
              <strong>{choice.title ?? choice.url}</strong>
              <span>{choice.url}</span>
            </button>
          {/each}
        </fieldset>
      {/if}

      <div class="actions">
        <button type="button" onclick={close}>Cancel</button>
        <button type="submit" class="primary" disabled={busy || !input.trim()}>
          {busy ? 'Looking…' : 'Add'}
        </button>
      </div>
    </form>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, black 40%, transparent);
  }
  .dialog {
    width: min(32rem, calc(100vw - 2rem));
    padding: var(--space-inset-xl);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
  }
  h2 {
    margin: 0 0 var(--space-stack-md);
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-lg);
  }
  form {
    display: grid;
    gap: var(--space-stack-sm);
  }
  label,
  legend {
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  input,
  select {
    height: var(--control-height);
    padding-inline: var(--space-inset-md);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-default);
    color: var(--color-text-default);
    font: inherit;
  }
  .error {
    margin: 0;
    color: var(--color-status-warning-text);
  }
  fieldset {
    display: grid;
    gap: var(--space-stack-sm);
    margin: 0;
    padding: 0;
    border: 0;
  }
  .choice {
    display: grid;
    gap: 2px;
    height: auto;
    padding: var(--space-inset-sm) var(--space-inset-md);
    text-align: start;
  }
  .choice span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-inline-sm);
    margin-block-start: var(--space-stack-sm);
  }
  button {
    height: var(--control-height);
    padding-inline: var(--space-inset-lg);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    color: var(--color-text-default);
    cursor: pointer;
  }
  .primary {
    border-color: transparent;
    background: var(--color-accent-default);
    color: var(--color-text-on-accent);
    font-weight: var(--font-weight-strong);
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
