<script lang="ts">
  import { hostOf } from '../format';
  import { hasContent, showLeadImage } from '../reader-media';
  import { app } from '../state.svelte';
  import EmptyState from './EmptyState.svelte';
  import Icon from './Icon.svelte';
  import ArticleHeader from './reader/ArticleHeader.svelte';
  import Attachments from './reader/Attachments.svelte';
  import { enhanceContent, type ViewerImage } from './reader/content';
  import ImageViewer from './reader/ImageViewer.svelte';
  import ReaderToolbar from './reader/ReaderToolbar.svelte';

  let scroller = $state<HTMLElement>();
  let heading = $state<HTMLElement>();
  const article = $derived(app.article);

  // Each Article starts at the top.
  $effect(() => {
    void article?.id;
    if (scroller) scroller.scrollTop = 0;
  });

  // The toolbar shows the title once the heading has scrolled under it.
  let headingHidden = $state(false);
  $effect(() => {
    if (!heading || !scroller) return;
    const observer = new IntersectionObserver(([entry]) => (headingHidden = !(entry?.isIntersecting ?? true)), {
      root: scroller,
      rootMargin: '-48px 0px 0px 0px',
    });
    observer.observe(heading);
    return () => observer.disconnect();
  });

  // An image enclosure is how photo feeds and podcasts carry their Lead Image; the header
  // already shows it, so it isn't listed again.
  const enclosures = $derived(
    article?.leadImage ? article.enclosures.filter((e) => !(e.mimeType ?? '').startsWith('image/')) : (article?.enclosures ?? []),
  );
  const next = $derived(app.selectedIndex === null ? undefined : app.rowAt(app.selectedIndex + 1));

  let viewer = $state<{ images: ViewerImage[]; index: number } | null>(null);
  $effect(() => {
    void article?.id;
    viewer = null;
  });

  function viewLead(src: string) {
    viewer = { images: [{ src, alt: '', caption: null, link: null, original: null }], index: 0 };
  }

  // Content HTML is sanitized in the Rust core. Links never navigate the app: they open in the
  // system browser. This covers the attachments too.
  function onClick(event: MouseEvent) {
    const link = (event.target as HTMLElement).closest('a');
    if (!link) return;
    event.preventDefault();
    const href = link.getAttribute('href');
    if (href && /^https?:\/\//.test(href)) void app.openExternal(href);
  }
</script>

<section class="reader" id="pane-reader" aria-label="Reader" tabindex="-1" bind:this={scroller}>
  {#if article}
    <ReaderToolbar {article} title={article.title} scrolled={headingHidden} />
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <article onclick={onClick}>
      <ArticleHeader {article} bind:heading onview={viewLead} />
      <Attachments {enclosures} />

      {#key article.contentHtml}
        <div class="content" use:enhanceContent={{ onView: (images, index) => (viewer = { images, index }) }} role="document">
          <!-- Sanitized by feedrico-core (ammonia allow-list) and constrained by the webview CSP. -->
          {@html article.contentHtml}
        </div>
      {/key}
      {#if !hasContent(article.contentHtml) && !showLeadImage(article.contentHtml, article.leadImage)}
        <p class="no-text">The feed sends only a title for this Article.</p>
      {/if}

      <footer class="end">
        {#if article.url}
          <button class="end-action" onclick={() => app.openInBrowser()}>
            <Icon name="external" size={16} />
            <span>Open on {hostOf(article.url) || 'the web'}</span>
          </button>
        {/if}
        {#if next}
          <button class="end-action next" onclick={() => void app.move(1)}>
            <span class="next-label">Next <kbd>j</kbd></span>
            <span class="next-title">{next.title}</span>
          </button>
        {/if}
      </footer>
    </article>
  {:else if (app.library?.feedCount ?? 0) > 0}
    <!-- With no Feeds there is nothing to pick: the Article list shows the first step. -->
    <EmptyState title="Pick an Article">
      Use <kbd>j</kbd> and <kbd>k</kbd> to move through the list.
    </EmptyState>
  {/if}
</section>

{#if viewer}
  <ImageViewer images={viewer.images} bind:index={viewer.index} onclose={() => (viewer = null)} />
{/if}

<style>
  .reader {
    min-width: 0;
    min-height: 0;
    height: 100%;
    overflow-y: auto;
    /* Scrolling stops here instead of chaining to the window once the Article ends. */
    overscroll-behavior-y: none;
    /* Focused links never hide under the sticky toolbar. */
    scroll-padding-block-start: calc(var(--icon-button-size) + var(--space-inset-md));
    background: var(--color-surface-raised);
    user-select: text;
    /* Wide images size themselves against the pane, not the text column (cqi units). */
    container-type: inline-size;
  }
  article {
    max-width: var(--reader-measure);
    margin-inline: auto;
    /* Narrow panes (List at the 880 px minimum window) give the text the room, not the margin. */
    padding: var(--space-stack-md) clamp(var(--space-inset-md), 6cqi, var(--reader-padding-inline)) calc(var(--reader-padding-block) * 2);
    box-sizing: content-box;
  }
  .no-text {
    color: var(--color-text-muted);
    font-style: italic;
  }
  .end {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(14rem, 100%), 1fr));
    gap: var(--space-stack-sm);
    margin-block-start: calc(var(--space-stack-lg) * 1.5);
    padding-block-start: var(--space-stack-lg);
    border-block-start: 1px solid var(--color-border-subtle);
  }
  .end-action {
    display: flex;
    align-items: center;
    gap: var(--space-inline-sm);
    min-height: calc(var(--control-height) * 1.5);
    padding: var(--space-inset-sm) var(--space-inset-md);
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-card);
    background: var(--color-surface-base);
    color: var(--color-text-default);
    font: inherit;
    font-size: var(--font-size-body);
    text-align: start;
    cursor: pointer;
    transition: background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .end-action:hover {
    background: var(--color-surface-hover);
  }
  .end-action.next {
    display: grid;
    gap: 0;
  }
  .next-label {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .next-title {
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    font-weight: var(--font-weight-medium);
  }

  /* Article content: editorial typography for arbitrary feed HTML. */
  .content {
    font-family: var(--reader-font-family);
    font-size: var(--reader-font-size);
    line-height: var(--reader-line-height);
    overflow-wrap: break-word;
  }
  .content :global(:where(p, ul, ol, blockquote, pre, figure, table)) {
    margin-block: 0 1.1em;
  }
  .content :global(:where(h1, h2, h3, h4, h5, h6)) {
    margin-block: 1.6em 0.5em;
    line-height: var(--font-line-height-heading);
    font-weight: var(--font-weight-heading);
  }
  .content :global(h1) {
    font-size: 1.5em;
  }
  .content :global(h2) {
    font-size: 1.3em;
  }
  .content :global(h3) {
    font-size: 1.15em;
  }
  .content :global(:where(h4, h5, h6)) {
    font-size: 1em;
  }
  .content :global(a) {
    color: var(--color-text-link);
    text-decoration-thickness: 1px;
    text-underline-offset: 0.18em;
    cursor: pointer;
  }

  /* Images: one treatment per size class (reader-media.ts). */
  .content :global(img) {
    max-width: 100%;
    height: auto;
  }
  .content :global(img.img-pixel),
  .content :global(img.missing) {
    display: none;
  }
  .content :global(img.img-inline) {
    display: inline;
    max-height: 1.6em;
    width: auto;
    vertical-align: middle;
  }
  .content :global(img.img-block) {
    display: block;
    margin: 1.4em auto;
    border-radius: var(--radius-card);
    background: var(--color-media-backdrop);
  }
  .content :global(img.img-block[role='button']) {
    cursor: zoom-in;
  }
  /* A whole photo fits on screen; tall comics and infographics scroll instead. */
  .content :global(img.img-cap) {
    max-height: 85vh;
    width: auto;
    object-fit: contain;
  }
  /* Strips and panoramas may use the pane width (up to their own), centred on the column. */
  .content :global(img.img-wide) {
    width: min(var(--img-width, 100cqi), calc(100cqi - 2 * var(--space-inset-lg)));
    max-width: none;
    max-height: none;
    margin-inline-start: 50%;
    translate: -50% 0;
  }
  /* Consecutive images (photo posts) sit closer together than images between paragraphs. */
  .content :global(img.img-block + img.img-block),
  .content :global(p:has(> img.img-block:only-child) + p:has(> img.img-block:only-child) > img) {
    margin-block-start: 0.6em;
  }
  .content :global(p:has(> img.img-block:only-child)) {
    margin-block: 0;
  }
  .content :global(.img-caption),
  .content :global(figcaption) {
    display: block;
    margin-block: -0.8em 1.4em;
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-small);
    line-height: var(--font-line-height-body);
    text-align: center;
  }
  .content :global(figure) {
    margin-inline: 0;
  }
  .content :global(figure img.img-block) {
    margin-block-end: 1.2em;
  }
  .content :global(.emoji) {
    font-style: normal;
  }

  .content :global(blockquote) {
    margin-inline: 0;
    padding-inline-start: 1em;
    border-inline-start: 3px solid var(--color-border-default);
    color: var(--color-text-muted);
    font-style: italic;
  }
  .content :global(:where(pre, code)) {
    font-family: var(--font-family-mono);
    font-size: 0.85em;
  }
  /* A shaded monospace box. Long lines scroll inside the block (overflow-x) and never break the
     reader width: wrapping is disabled inside pre, so the block grows its own scrollbar instead. */
  .content :global(pre) {
    position: relative;
    max-width: 100%;
    overflow-x: auto;
    overflow-wrap: normal;
    padding: var(--space-inset-md);
    /* Room for the copy button in the top-right corner. */
    padding-inline-end: calc(var(--space-inset-md) + 4.5em);
    border-radius: var(--radius-card);
    background: var(--color-surface-sunken);
    line-height: var(--font-line-height-body);
  }
  .content :global(pre code) {
    display: block;
    white-space: pre;
    overflow-wrap: normal;
    padding: 0;
    border-radius: 0;
    background: none;
  }
  /* Bundled highlighter output (issue #77). Colors are code tokens; the token build enforces
     WCAG 2.2 AA on the code block background in both themes. */
  .content :global(.tok-keyword) {
    color: var(--color-code-keyword);
  }
  .content :global(.tok-string) {
    color: var(--color-code-string);
  }
  .content :global(.tok-comment) {
    color: var(--color-code-comment);
    font-style: italic;
  }
  .content :global(.tok-number) {
    color: var(--color-code-number);
  }
  .content :global(.tok-function) {
    color: var(--color-code-function);
  }
  /* Copy button per pre block: a real <button>, so it is keyboard-operable (Tab, Enter/Space)
     and gets the global :focus-visible ring. */
  .content :global(.code-copy) {
    position: absolute;
    top: var(--space-inset-xs);
    right: var(--space-inset-xs);
    padding: var(--space-inset-xs) var(--space-inset-sm);
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-pill);
    background: var(--color-surface-raised);
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-caption);
    line-height: 1.2;
    cursor: pointer;
  }
  .content :global(.code-copy:hover) {
    background: var(--color-surface-hover);
    color: var(--color-text-default);
  }
  .content :global(.code-copy.copied) {
    color: var(--color-status-success-text);
  }
  .content :global(:not(pre) > code) {
    padding: 0.1em 0.3em;
    border-radius: var(--radius-control);
    background: var(--color-surface-sunken);
  }
  .content :global(hr) {
    margin-block: 2em;
    border: 0;
    border-block-start: 1px solid var(--color-border-default);
  }
  .content :global(table) {
    display: block;
    overflow-x: auto;
    border-collapse: collapse;
    font-family: var(--font-family-ui);
    font-size: var(--font-size-body);
  }
  .content :global(:where(th, td)) {
    padding: var(--space-inset-xs) var(--space-inset-sm);
    border: 1px solid var(--color-border-subtle);
  }

  /* MathML equations (issue #78, ADR 0004): the webview renders them natively and screen
     readers voice them (implicit role "math", accessible name from alttext). Glyphs inherit
     the reader font size, so equations scale with the reader typography, and use the text
     color token, which the token build AA-checks against the reader surface in both themes. */
  .content :global(math) {
    color: var(--color-text-default);
    overflow-wrap: normal;
  }
  /* Display math gets its own centered line; a long equation scrolls instead of breaking
     the measure. */
  .content :global(math[display="block"]) {
    display: block;
    margin-block: 1.1em;
    padding-block: 0.2em;
    text-align: center;
    overflow-x: auto;
  }

  /* Cards for what the reader can't show inline: uncached images and embeds. */
  .content :global(:is(.embed-link, .remote-image)) {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-inline-xs) var(--space-inline-sm);
    margin-block: 1.4em;
    padding: var(--space-inset-md);
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-card);
    background: var(--color-surface-sunken);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-body);
    line-height: var(--font-line-height-body);
  }
  .content :global(:is(.embed-link, .remote-image))::before {
    content: '';
    flex: none;
    width: 20px;
    height: 20px;
    background: var(--color-text-muted);
    mask: var(--card-icon) center / contain no-repeat;
  }
  .content :global(.embed-link) {
    --card-icon: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20' fill='none' stroke='black' stroke-width='1.5' stroke-linejoin='round'%3E%3Cpath d='M3.5 5.5h9v9h-9zM12.5 8.5l4-2.5v8l-4-2.5'/%3E%3C/svg%3E");
  }
  .content :global(.remote-image) {
    --card-icon: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20' fill='none' stroke='black' stroke-width='1.5' stroke-linejoin='round' stroke-linecap='round'%3E%3Cpath d='M3.5 4.5h13v11h-13zM3.5 13l4-4 3 3 2-2 4 4M3 3l14 14'/%3E%3C/svg%3E");
  }
  .content :global(.remote-image-text) {
    display: grid;
    flex: 1 1 12rem;
    min-width: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .content :global(.remote-image-text strong) {
    color: var(--color-text-default);
    font-weight: var(--font-weight-medium);
  }
  .content :global(.remote-image-action) {
    font-size: var(--font-size-small);
  }
  .content :global(a:has(> .remote-image)) {
    text-decoration: none;
  }
  kbd {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-caption);
    padding: 0 var(--space-inset-xs);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-sunken);
  }
</style>
