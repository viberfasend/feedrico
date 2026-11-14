<script lang="ts">
  import { mediaUrl, type ArticleRow } from '../../api';
  import { articleOptionLabel } from '../../aria';
  import { shortDate } from '../../format';
  import { failedImages, hasLeadImage } from '../../images.svelte';
  import { app } from '../../state.svelte';
  import { tokens } from '../../tokens/runtime.svelte';
  import FeedIcon from '../FeedIcon.svelte';
  import Icon from '../Icon.svelte';
  import VirtualRows from '../VirtualRows.svelte';

  let { label }: { label: string } = $props();

  // An editorial front page made of repeating bands: 1 lead, 2 medium, 4 small stories.
  // Bands have a fixed height, so the page is virtualized like any list.
  const PER_BAND = 7;
  const padding = $derived(tokens.px('--grid-padding'));
  const gap = $derived(tokens.px('--magazine-gap'));
  const bandHeight = $derived(tokens.px('--magazine-band-height') || 700);
  const smallRow = $derived(tokens.px('--magazine-small-row-height') || 132);

  $effect(() => {
    app.gridColumns = 1;
  });

  type Slot = { index: number; article: ArticleRow | undefined };

  /** The lead slot takes the first story in the band with a Lead Image; the rest keep order. */
  function bandSlots(band: number): Slot[] {
    const slots: Slot[] = [];
    for (let i = band * PER_BAND; i < Math.min((band + 1) * PER_BAND, app.total); i++) {
      slots.push({ index: i, article: app.rowAt(i) });
    }
    const lead = slots.findIndex((s) => hasLeadImage(s.article?.leadImage));
    if (lead > 0) slots.unshift(...slots.splice(lead, 1));
    return slots;
  }

  const kind = (position: number) => (position === 0 ? 'lead' : position < 3 ? 'medium' : 'small');
</script>

<VirtualRows rowHeight={bandHeight + gap} perRow={PER_BAND} {padding} {label}>
  {#snippet row(band)}
    {@const __ = failedImages.size}
    <div
      class="band"
      style:height="{bandHeight}px"
      style:gap="{gap}px"
      style:grid-template-rows="1fr 1fr {smallRow}px"
    >
      {#each bandSlots(band) as { index, article }, position (index)}
        {#if article}
          {@const withImage = kind(position) !== 'small' && hasLeadImage(article.leadImage)}
          {@const imageMissing = kind(position) !== 'small' && !!article.leadImage && !withImage}
          <button
            class="story {kind(position)}"
            class:unread={!article.isRead}
            class:selected={app.selectedIndex === index}
            class:with-image={withImage}
            role="option"
            aria-selected={app.selectedIndex === index}
            aria-setsize={app.total}
            aria-posinset={index + 1}
            aria-label={articleOptionLabel(article)}
            onclick={() => app.selectIndex(index)}
          >
            {#if withImage}
              <div class="visual">
                <img
                  src={mediaUrl(article.leadImage!)}
                  alt=""
                  decoding="async"
                  onerror={() => failedImages.add(article.leadImage!)}
                />
              </div>
            {/if}
            <div class="text">
              <div class="meta">
                <FeedIcon feedId={article.feedId} title={article.feedTitle} favicon={article.favicon} />
                <span class="feed">{article.feedTitle}</span>
                {#if imageMissing}<span class="missing"><Icon name="image" size={12} label="Image not cached" /></span>{/if}
                <time>{shortDate(article.articleDate)}</time>
              </div>
              <div class="title">{article.title}</div>
              {#if kind(position) !== 'small'}
                <div class="excerpt">{article.excerpt}</div>
              {/if}
            </div>
          </button>
        {:else}
          <div class="story {kind(position)} skeleton"></div>
        {/if}
      {/each}
    </div>
  {/snippet}
</VirtualRows>

<style>
  .band {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    padding-block-end: var(--space-stack-md);
    border-block-end: 1px solid var(--color-border-subtle);
  }
  .story {
    display: grid;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    padding: 0;
    border: 0;
    border-radius: var(--radius-card);
    background: none;
    color: inherit;
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .story:hover .title {
    text-decoration: underline;
    text-decoration-thickness: 1px;
    text-underline-offset: 0.15em;
  }
  .story.selected {
    outline: 2px solid var(--color-border-focus);
    outline-offset: 2px;
  }
  .lead {
    grid-column: 1 / 3;
    grid-row: 1 / 3;
    grid-template-rows: minmax(0, 1fr) auto;
  }
  .medium {
    grid-column: 3 / 5;
  }
  .medium.with-image {
    grid-template-columns: 2fr 3fr;
    gap: var(--space-inline-md);
  }
  .small {
    grid-row: 3;
    align-content: start;
    padding-block-start: var(--space-stack-sm);
    border-block-start: 1px solid var(--color-border-subtle);
    border-radius: 0;
  }
  .visual {
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--color-surface-sunken);
    min-height: 0;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .text {
    display: grid;
    align-content: start;
    gap: var(--space-stack-xs);
    min-height: 0;
    overflow: hidden;
  }
  .lead .text {
    padding-block-start: var(--space-stack-sm);
  }
  /* Text hero: same footprint as an image hero, filled with title and a long excerpt. */
  .lead:not(.with-image) {
    grid-template-rows: minmax(0, 1fr);
    padding: var(--space-inset-xl);
    border: 1px solid var(--color-border-subtle);
    background: var(--color-surface-raised);
  }
  .lead:not(.with-image) .text {
    grid-template-rows: auto auto minmax(0, 1fr);
    gap: var(--space-stack-md);
    padding-block-start: 0;
  }
  .medium:not(.with-image) .text {
    grid-template-rows: auto auto minmax(0, 1fr);
  }
  .missing {
    display: inline-flex;
    flex: none;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--space-inline-sm);
    min-width: 0;
    color: var(--color-text-accent);
    font-size: var(--font-size-caption);
    font-weight: var(--font-weight-medium);
  }
  .feed {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  time {
    flex: none;
    color: var(--color-text-muted);
    font-weight: var(--font-weight-regular);
  }
  .title {
    font-family: var(--font-family-reading);
    font-weight: var(--font-weight-heading);
    line-height: var(--font-line-height-heading);
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    overflow: hidden;
    -webkit-line-clamp: 3;
    line-clamp: 3;
  }
  .lead .title {
    font-size: var(--font-size-title-lg);
    letter-spacing: -0.01em;
  }
  .lead:not(.with-image) .title {
    font-size: var(--font-size-title-lg);
    -webkit-line-clamp: 4;
    line-clamp: 4;
  }
  .medium .title {
    font-size: var(--font-size-title-sm);
  }
  .small .title {
    font-size: var(--font-size-body);
    -webkit-line-clamp: 3;
    line-clamp: 3;
  }
  .story:not(.unread) .title {
    color: var(--color-text-muted);
  }
  .excerpt {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
    line-height: var(--font-line-height-body);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    overflow: hidden;
    -webkit-line-clamp: 3;
    line-clamp: 3;
  }
  .lead .excerpt {
    font-size: var(--font-size-body);
  }
  .lead:not(.with-image) .excerpt {
    min-height: 0;
    -webkit-line-clamp: 12;
    line-clamp: 12;
  }
  .skeleton {
    background: var(--color-surface-hover);
  }
</style>
