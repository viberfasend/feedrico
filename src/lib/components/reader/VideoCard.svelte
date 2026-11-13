<script lang="ts">
  // The video card for video Feeds and video embeds (#75): the cached thumbnail, the title,
  // duration, view count and an "Open in browser" link. No live iframe: reading an Article
  // makes no network request (spec v1: no inline player; images only from the local cache).
  import { mediaUrl, videoWatchUrl, type ArticleDetail } from '../../api';
  import Icon from '../Icon.svelte';
  import { remoteImageCard } from './content';

  let { article }: { article: ArticleDetail } = $props();

  const video = $derived(article.video ?? null);
  const watchUrl = $derived(video ? videoWatchUrl(video) : '');
  const hostLabel = $derived(video?.host === 'vimeo' ? 'vimeo.com' : 'youtube.com');
  // The Feed's own thumbnail (media:thumbnail) is the Lead Image and cached at fetch time.
  const thumb = $derived(video?.thumbnail && article.leadImage ? mediaUrl(article.leadImage) : null);

  /** 754 → "12:34", 3725 → "1:02:05". */
  function formatDuration(totalSeconds: number): string {
    const s = Math.max(0, Math.round(totalSeconds));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = String(s % 60).padStart(2, '0');
    return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`;
  }

  // A thumbnail that isn't cached (over 5 MB, evicted) stays a remote link (spec): the core
  // keeps its original URL in the inert data-remote attribute.
  function onThumbError(event: Event) {
    const img = event.target;
    if (!(img instanceof HTMLImageElement)) return;
    const remote = img.dataset.remote;
    if (remote && /^https?:\/\//.test(remote)) img.replaceWith(remoteImageCard(img, remote));
    else img.classList.add('missing');
  }
</script>

{#if video}
  <section class="video-card" aria-label="Video">
    {#if thumb}
      <img class="video-thumb" src={thumb} data-remote={video.thumbnail} alt="" onerror={onThumbError} />
    {/if}
    <div class="video-body">
      <span class="video-kind"><Icon name="video" size={14} /> Video · {hostLabel}</span>
      <span class="video-title">{article.title || 'Untitled'}</span>
      {#if video.duration != null || video.views != null}
        <span class="video-stats">
          {#if video.duration != null}<span>{formatDuration(video.duration)}</span>{/if}
          {#if video.views != null}<span>{video.views.toLocaleString('en-US')} views</span>{/if}
        </span>
      {/if}
      <a class="video-open" href={watchUrl}>
        <Icon name="external" size={14} />
        Open in browser
      </a>
    </div>
  </section>
{/if}

<style>
  .video-card {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-inset-md);
    margin-block-end: var(--space-stack-lg);
    padding: var(--space-inset-md);
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-card);
    background: var(--color-surface-sunken);
  }
  .video-thumb {
    flex: none;
    width: 168px;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    border-radius: var(--radius-control);
    background: var(--color-media-backdrop);
  }
  .video-card :global(img.missing) {
    display: none;
  }
  .video-body {
    display: grid;
    gap: var(--space-stack-xs);
    align-content: start;
    min-width: 0;
    flex: 1 1 14rem;
  }
  .video-kind {
    display: inline-flex;
    align-items: center;
    gap: var(--space-inline-xs);
    color: var(--color-text-accent);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-small);
    font-weight: var(--font-weight-medium);
  }
  .video-title {
    font-family: var(--reader-font-family);
    font-size: var(--font-size-body);
    font-weight: var(--font-weight-heading);
    line-height: var(--font-line-height-body);
    overflow-wrap: anywhere;
  }
  .video-stats {
    display: flex;
    flex-wrap: wrap;
    gap: 0 var(--space-inline-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-small);
  }
  .video-stats > :not(:first-child)::before {
    content: '·';
    margin-inline-end: var(--space-inline-xs);
  }
  .video-open {
    display: inline-flex;
    align-items: center;
    gap: var(--space-inline-xs);
    justify-self: start;
    color: var(--color-text-link);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-small);
    text-decoration-thickness: 1px;
    text-underline-offset: 0.18em;
  }
  /* The remote-link fallback card for a thumbnail that isn't cached (same treatment as in
     the content, scoped here because this card sits outside .content). */
  .video-card :global(.remote-image) {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-inline-xs) var(--space-inline-sm);
    padding: var(--space-inset-sm) var(--space-inset-md);
    border: 1px dashed var(--color-border-default);
    border-radius: var(--radius-card);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-small);
  }
  .video-card :global(.remote-image-text) {
    display: grid;
    flex: 1 1 12rem;
    min-width: 0;
    color: var(--color-text-muted);
  }
  .video-card :global(.remote-image-text strong) {
    color: var(--color-text-default);
    font-weight: var(--font-weight-medium);
  }
</style>
