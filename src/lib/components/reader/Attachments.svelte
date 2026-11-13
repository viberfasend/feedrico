<script lang="ts">
  // Enclosures as calm, readable cards: what it is, how big, where it opens. Nothing plays or
  // downloads inline: opening one is an explicit click that hands it to the system.
  import { fileSize, hostOf } from '../../format';
  import { enclosureName, formatDuration, formatLabel, type EnclosureKind } from '../../reader-media';
  import type { Enclosure } from '../../api';
  import Icon from '../Icon.svelte';

  let { enclosures }: { enclosures: Enclosure[] } = $props();

  const KIND_LABEL: Record<EnclosureKind, string> = { audio: 'Audio', video: 'Video', image: 'Image', file: 'File' };
  const items = $derived(
    enclosures.map((e) => {
      // The core derives the kind from the MediaRSS medium / MIME type / extension (#76); a
      // media:player page is the "open" target when there is no direct media URL.
      const kind: EnclosureKind = e.kind === 'audio' || e.kind === 'video' ? e.kind : 'file';
      const href = e.playerUrl ?? e.url;
      const label = e.playerUrl ? 'Player' : KIND_LABEL[kind];
      return {
        url: href,
        kind,
        name: enclosureName(href),
        details: [label, formatLabel(e.mimeType, href), formatDuration(e.durationSecs), fileSize(e.length)].filter(Boolean).join(' · '),
        host: hostOf(href),
      };
    }),
  );
</script>

{#if items.length}
  <ul class="attachments" aria-label="Attachments">
    {#each items as item (item.url)}
      <li>
        <a href={item.url} title={`Open on ${item.host}`}>
          <span class="icon"><Icon name={item.kind === 'file' ? 'file' : item.kind} size={18} /></span>
          <span class="text">
            <span class="name">{item.name}</span>
            <span class="details">{item.details}</span>
          </span>
          <span class="open"><Icon name="external" size={14} /><span class="sr-only">Opens on {item.host}</span></span>
        </a>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .attachments {
    list-style: none;
    padding: 0;
    margin: 0 0 var(--space-stack-lg);
    display: grid;
    gap: var(--space-stack-xs);
  }
  a {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-inline-sm);
    padding: var(--space-inset-sm) var(--space-inset-md);
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-card);
    background: var(--color-surface-base);
    color: var(--color-text-default);
    text-decoration: none;
    transition: background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  a:hover {
    background: var(--color-surface-hover);
  }
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: calc(var(--control-height) * 1.1);
    height: calc(var(--control-height) * 1.1);
    border-radius: var(--radius-control);
    background: var(--color-surface-sunken);
    color: var(--color-text-accent);
  }
  .text {
    display: grid;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    font-size: var(--font-size-body);
    font-weight: var(--font-weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .details {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .open {
    color: var(--color-text-muted);
  }
</style>
