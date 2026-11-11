<script lang="ts">
  import { fileSize, hostOf, longDate } from '../format';
  import { app } from '../state.svelte';
  import EmptyState from './EmptyState.svelte';
  import Icon from './Icon.svelte';

  let scroller = $state<HTMLElement>();
  const article = $derived(app.article);

  // Each Article starts at the top.
  $effect(() => {
    void article?.id;
    if (scroller) scroller.scrollTop = 0;
  });

  // Content HTML is sanitized in the Rust core. Links never navigate the app: they open in the
  // system browser.
  function onClick(event: MouseEvent) {
    const link = (event.target as HTMLElement).closest('a');
    if (!link) return;
    event.preventDefault();
    const href = link.getAttribute('href');
    if (href && /^https?:\/\//.test(href)) void app.openExternal(href);
  }

  // Images load only from the local cache. One that isn't cached (over 5 MB, evicted, not
  // fetched yet) "stays a remote link" (spec): the core keeps its original URL in the inert
  // data-remote attribute. Articles stored before that just hide the broken image.
  function linkMissingImages(node: HTMLElement) {
    const onError = (event: Event) => {
      const img = event.target;
      if (!(img instanceof HTMLImageElement)) return;
      const remote = img.dataset.remote;
      if (remote && /^https?:\/\//.test(remote)) img.replaceWith(remoteImage(img, remote));
      else img.classList.add('missing');
    };
    node.addEventListener('error', onError, true);
    return { destroy: () => node.removeEventListener('error', onError, true) };
  }

  function remoteImage(img: HTMLImageElement, remote: string): HTMLElement {
    const card = document.createElement('span');
    card.className = 'remote-image';
    const label = `Image on ${hostOf(remote)}${img.alt ? `: ${img.alt}` : ''}`;
    // Inside a link already: that link stays the target, links can't nest.
    if (img.closest('a')) {
      card.textContent = label;
    } else {
      const link = document.createElement('a');
      link.href = remote;
      link.textContent = label;
      card.append(link);
    }
    return card;
  }
</script>

<section class="reader" id="pane-reader" aria-label="Reader" tabindex="-1" bind:this={scroller}>
  {#if article}
    <article>
      <header class="head">
        {#if app.isGrid}
          <button class="close" onclick={() => app.closeReader()} aria-label="Close reader" title="Close (Esc)">
            <Icon name="close" size={18} />
          </button>
        {/if}
        <div class="source">
          <span>{article.feedTitle}</span>
          {#if article.author}<span>· {article.author}</span>{/if}
        </div>
        <h1>{article.title}</h1>
        <div class="byline">
          <time datetime={new Date(article.articleDate * 1000).toISOString()}>{longDate(article.articleDate)}</time>
        </div>
        <div class="actions">
          <button
            class="action"
            class:active={article.isStarred}
            aria-pressed={article.isStarred}
            title="Star (s)"
            onclick={() => app.toggleStar()}
          >
            <Icon name="star" size={16} filled={article.isStarred} />
            {article.isStarred ? 'Starred' : 'Star'}
          </button>
          <button class="action" title="Toggle read (m)" onclick={() => app.toggleRead()}>
            <Icon name={article.isRead ? 'circle' : 'check'} size={16} />
            {article.isRead ? 'Mark unread' : 'Mark read'}
          </button>
          {#if article.url}
            <button class="action" title="Open in browser (o)" onclick={() => app.openInBrowser()}>
              <Icon name="external" size={16} />
              {hostOf(article.url) || 'Open in browser'}
            </button>
          {/if}
        </div>
      </header>

      {#if article.enclosures.length}
        <ul class="enclosures">
          {#each article.enclosures as enclosure (enclosure.url)}
            <li>
              <a href={enclosure.url}>
                <Icon name="attachment" size={16} />
                <span>{enclosure.url.split('/').pop()?.split('?')[0] || enclosure.url}</span>
                <small>{[enclosure.mimeType, fileSize(enclosure.length)].filter(Boolean).join(' · ')}</small>
              </a>
            </li>
          {/each}
        </ul>
      {/if}

      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
      <div class="content" onclick={onClick} use:linkMissingImages role="document">
        <!-- Sanitized by feedrico-core (ammonia allow-list) and constrained by the webview CSP. -->
        {@html article.contentHtml}
      </div>
    </article>
  {:else}
    <EmptyState title="Pick an Article">
      Use <kbd>j</kbd> and <kbd>k</kbd> to move through the list.
    </EmptyState>
  {/if}
</section>

<style>
  .reader {
    min-width: 0;
    min-height: 0;
    height: 100%;
    overflow-y: auto;
    background: var(--color-surface-raised);
    user-select: text;
  }
  article {
    max-width: var(--reader-measure);
    margin-inline: auto;
    padding: var(--reader-padding-block) var(--reader-padding-inline) calc(var(--reader-padding-block) * 3);
    box-sizing: content-box;
  }
  .close {
    justify-self: end;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-height);
    height: var(--control-height);
    margin-block-end: calc(var(--space-stack-sm) * -1);
    border: 0;
    border-radius: var(--radius-pill);
    background: var(--color-surface-sunken);
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .close:hover {
    color: var(--color-text-default);
  }
  .head {
    display: grid;
    gap: var(--space-stack-sm);
    padding-block-end: var(--space-stack-lg);
    margin-block-end: var(--space-stack-lg);
    border-block-end: 1px solid var(--color-border-subtle);
  }
  .source {
    display: flex;
    gap: var(--space-inline-xs);
    color: var(--color-text-accent);
    font-size: var(--font-size-small);
    font-weight: var(--font-weight-medium);
  }
  h1 {
    font-family: var(--reader-font-family);
    font-size: var(--font-size-title-lg);
    font-weight: var(--font-weight-heading);
    line-height: var(--font-line-height-heading);
    letter-spacing: -0.01em;
    text-wrap: balance;
  }
  .byline {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-inline-xs);
    margin-block-start: var(--space-stack-xs);
  }
  .action {
    display: inline-flex;
    align-items: center;
    gap: var(--space-inline-xs);
    height: var(--control-height);
    padding-inline: var(--space-inset-md);
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-pill);
    background: var(--color-surface-base);
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
    cursor: pointer;
    transition:
      background-color var(--motion-duration-fast) var(--motion-easing-standard),
      color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .action:hover {
    background: var(--color-surface-hover);
    color: var(--color-text-default);
  }
  .action.active {
    color: var(--color-text-accent);
    border-color: var(--color-border-default);
  }
  .enclosures {
    list-style: none;
    padding: 0;
    margin: 0 0 var(--space-stack-lg);
    display: grid;
    gap: var(--space-stack-xs);
  }
  .enclosures a {
    display: flex;
    align-items: center;
    gap: var(--space-inline-sm);
    padding: var(--space-inset-sm) var(--space-inset-md);
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-card);
    color: var(--color-text-default);
    text-decoration: none;
  }
  .enclosures span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .enclosures small {
    color: var(--color-text-muted);
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
  .content :global(img) {
    display: block;
    max-width: 100%;
    height: auto;
    margin-block: 1.2em;
    border-radius: var(--radius-card);
  }
  .content :global(img.missing) {
    display: none;
  }
  .content :global(figure) {
    margin-inline: 0;
  }
  .content :global(figcaption) {
    color: var(--color-text-muted);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-small);
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
  .content :global(pre) {
    overflow-x: auto;
    padding: var(--space-inset-md);
    border-radius: var(--radius-card);
    background: var(--color-surface-sunken);
    line-height: var(--font-line-height-body);
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
  .content :global(.remote-image) {
    display: block;
    margin-block: 1.2em;
  }
  .content :global(:is(.embed-link, .remote-image)) {
    padding: var(--space-inset-md);
    border: 1px dashed var(--color-border-default);
    border-radius: var(--radius-card);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-body);
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
