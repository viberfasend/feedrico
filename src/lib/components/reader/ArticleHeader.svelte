<script lang="ts">
  // Who wrote it, what it is, when, and how long it takes: in that order. The Lead Image leads the
  // Article when the content doesn't show it already (summary-only feeds, photo and podcast
  // enclosures), so what the card showed is also what the reader shows.
  import { mediaUrl, type ArticleDetail } from '../../api';
  import { longDate } from '../../format';
  import { readingMinutes, showLeadImage } from '../../reader-media';
  import FeedIcon from '../FeedIcon.svelte';

  let {
    article,
    heading = $bindable(),
    onview,
  }: {
    article: ArticleDetail;
    heading?: HTMLElement;
    onview: (src: string) => void;
  } = $props();

  const minutes = $derived(readingMinutes(article.contentHtml));
  // The video card shows the Feed's own thumbnail; the Lead Image must not show it twice (spec).
  const lead = $derived(
    article.leadImage && !article.video?.thumbnail && showLeadImage(article.contentHtml, article.leadImage)
      ? mediaUrl(article.leadImage)
      : null,
  );
  let leadFailed = $state(false);
  $effect(() => {
    void article.id;
    leadFailed = false;
  });
</script>

<header class="head">
  <div class="source" title={article.feedTitle}>
    <FeedIcon feedId={article.feedId} title={article.feedTitle} favicon={article.favicon} />
    <span class="feed">{article.feedTitle}</span>
  </div>
  <h1 bind:this={heading}>{article.title || 'Untitled'}</h1>
  <p class="byline">
    {#if article.author}<span class="author">{article.author}</span>{/if}
    <time datetime={new Date(article.articleDate * 1000).toISOString()}>{longDate(article.articleDate)}</time>
    {#if minutes !== null && minutes > 1}<span>{minutes} min read</span>{/if}
  </p>
</header>

{#if lead && !leadFailed}
  <figure class="lead">
    <!-- Space and Enter belong to the button here, not to paging (App skips buttons). -->
    <button type="button" aria-label="View Lead Image" onclick={() => onview(lead)}>
      <img src={lead} alt="" onerror={() => (leadFailed = true)} />
    </button>
  </figure>
{/if}

<style>
  .head {
    display: grid;
    gap: var(--space-stack-sm);
    margin-block-end: var(--space-stack-lg);
  }
  .source {
    display: flex;
    align-items: center;
    gap: var(--space-inline-xs);
    min-width: 0;
    color: var(--color-text-accent);
    font-size: var(--font-size-small);
    font-weight: var(--font-weight-medium);
  }
  .feed {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  h1 {
    margin: 0;
    font-family: var(--reader-font-family);
    font-size: var(--font-size-title-lg);
    font-weight: var(--font-weight-heading);
    line-height: var(--font-line-height-heading);
    letter-spacing: -0.01em;
    text-wrap: balance;
    /* "anywhere", not "break-word": a long compound word must not widen the grid column. */
    overflow-wrap: anywhere;
  }
  .byline {
    display: flex;
    flex-wrap: wrap;
    gap: 0 var(--space-inline-xs);
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .byline > :not(:first-child)::before {
    content: '·';
    margin-inline-end: var(--space-inline-xs);
  }
  .author {
    color: var(--color-text-default);
  }
  .lead {
    margin: 0 0 var(--space-stack-lg);
  }
  .lead button {
    display: block;
    width: 100%;
    padding: 0;
    border: 0;
    border-radius: var(--radius-card);
    background: none;
    cursor: zoom-in;
  }
  .lead img {
    display: block;
    max-width: 100%;
    max-height: 50vh;
    width: auto;
    height: auto;
    margin-inline: auto;
    border-radius: var(--radius-card);
    background: var(--color-media-backdrop);
  }
</style>
