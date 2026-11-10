<script lang="ts">
  import { api, type Discovery, type FeedCandidate } from '../api';
  import { app, errorMessage } from '../state.svelte';
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Input from '../ui/Input.svelte';

  let input = $state('');
  let folderId = $state<number | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let choices = $state<FeedCandidate[] | null>(null);

  function close() {
    app.addFeedOpen = false;
  }

  // The address field gets focus when the dialog opens (after the dialog's own focus).
  $effect(() => {
    if (app.addFeedOpen) {
      const t = setTimeout(() => document.getElementById('add-feed-url')?.focus(), 0);
      return () => clearTimeout(t);
    }
  });

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
</script>

<Dialog bind:open={app.addFeedOpen} title="Add Feed">
  <form onsubmit={submit}>
    <label for="add-feed-url">Feed or website address</label>
    <Input
      bind:value={input}
      id="add-feed-url"
      type="text"
      inputmode="url"
      autocomplete="off"
      spellcheck={false}
      placeholder="example.com or https://example.com/feed.xml"
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
      <Button type="button" onclick={close}>Cancel</Button>
      <Button variant="primary" type="submit" disabled={busy || !input.trim()}>
        {busy ? 'Looking…' : 'Add'}
      </Button>
    </div>
  </form>
</Dialog>

<style>
  form {
    display: grid;
    gap: var(--space-stack-sm);
  }
  label,
  legend {
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  select {
    height: var(--control-height);
    padding-inline: var(--space-inset-md);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-base);
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
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-raised);
    color: var(--color-text-default);
    text-align: start;
    cursor: pointer;
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
</style>
