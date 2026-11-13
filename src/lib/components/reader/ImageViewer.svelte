<script lang="ts">
  // Full-window image viewer for the pictures in an Article. Images come from the local cache
  // only; "Open original" and "Open link" are the only ways out to the network, and only on
  // click (spec: no request when reading).
  import { hostOf } from '../../format';
  import { app } from '../../state.svelte';
  import { tokens } from '../../tokens/runtime.svelte';
  import { fade } from 'svelte/transition';
  import Icon from '../Icon.svelte';
  import type { ViewerImage } from './content';

  let {
    images,
    index = $bindable(0),
    onclose,
  }: { images: ViewerImage[]; index?: number; onclose: () => void } = $props();

  const image = $derived(images[index]);
  /** Fit to the window, or the image's real size (scroll to pan). */
  let actualSize = $state(false);
  let stage = $state<HTMLElement>();

  $effect(() => {
    void index;
    actualSize = false;
  });

  function go(delta: number) {
    const next = index + delta;
    if (next >= 0 && next < images.length) index = next;
  }

  /** Moves the dialog to <body>, so no pane or slide-over clips or stacks it. */
  function portal(node: HTMLElement) {
    const opener = document.activeElement as HTMLElement | null;
    document.body.append(node);
    node.focus({ preventScroll: true });
    return {
      destroy: () => {
        node.remove();
        opener?.focus({ preventScroll: true });
      },
    };
  }

  // The viewer owns the keyboard while open: Esc closes the viewer, not the reader behind it, and
  // j/k/s/m don't act on the Article underneath.
  function onKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    const keys: Record<string, () => void> = {
      Escape: onclose,
      ArrowLeft: () => go(-1),
      ArrowRight: () => go(1),
      z: () => (actualSize = !actualSize),
    };
    if (event.key === 'Tab') return trapFocus(event);
    const action = keys[event.key];
    if (action) {
      event.preventDefault();
      action();
    }
  }

  function trapFocus(event: KeyboardEvent) {
    const root = event.currentTarget as HTMLElement;
    const focusable = [...root.querySelectorAll<HTMLElement>('button:not(:disabled)')];
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (!first || !last) return;
    if (event.shiftKey && (document.activeElement === first || document.activeElement === root)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
</script>

<div
  class="viewer"
  role="dialog"
  aria-modal="true"
  aria-label={image?.alt ? `Image: ${image.alt}` : 'Image'}
  tabindex="-1"
  onkeydown={onKeydown}
  use:portal
  transition:fade={{ duration: tokens.px('--motion-duration-fast') }}
>
  <div class="bar">
    <span class="count" aria-live="polite">
      {#if images.length > 1}{index + 1} / {images.length}{/if}
    </span>
    <div class="tools">
      {#if image?.link && /^https?:\/\//.test(image.link)}
        <button class="tool text" onclick={() => app.openExternal(image.link!)}>
          <Icon name="external" size={16} /> Open link on {hostOf(image.link)}
        </button>
      {/if}
      {#if image?.original}
        <button class="tool text" onclick={() => app.openExternal(image.original!)}>
          <Icon name="external" size={16} /> Original on {hostOf(image.original)}
        </button>
      {/if}
      <button class="tool" aria-label={actualSize ? 'Fit to window' : 'Actual size'} aria-pressed={actualSize} title={actualSize ? 'Fit to window (z)' : 'Actual size (z)'} onclick={() => (actualSize = !actualSize)}>
        <Icon name={actualSize ? 'shrink' : 'expand'} size={18} />
      </button>
      <button class="tool" aria-label="Close image" title="Close (Esc)" onclick={onclose}>
        <Icon name="close" size={18} />
      </button>
    </div>
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="stage" class:actual={actualSize} bind:this={stage} onclick={(e) => e.target === stage && onclose()}>
    {#if image}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_click_events_have_key_events -->
      <img src={image.src} alt={image.alt} onclick={() => (actualSize = !actualSize)} />
    {/if}
  </div>

  {#if images.length > 1}
    <button class="nav prev" aria-label="Previous image" title="Previous image (←)" disabled={index === 0} onclick={() => go(-1)}>
      <Icon name="chevron" size={22} />
    </button>
    <button class="nav next" aria-label="Next image" title="Next image (→)" disabled={index === images.length - 1} onclick={() => go(1)}>
      <Icon name="chevron" size={22} />
    </button>
  {/if}

  {#if image?.caption}
    <p class="caption">{image.caption}</p>
  {/if}
</div>

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: var(--layer-viewer);
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    background: var(--color-media-viewer);
    color: var(--color-media-caption);
    outline: none;
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-inline-sm);
    padding: var(--space-inset-sm) var(--space-inset-md);
  }
  .count {
    font-size: var(--font-size-small);
    font-variant-numeric: tabular-nums;
    opacity: 0.85;
  }
  .tools {
    display: flex;
    gap: var(--space-inline-xs);
  }
  .tool,
  .nav {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-inline-xs);
    min-width: var(--control-height);
    height: var(--control-height);
    border: 0;
    border-radius: var(--radius-pill);
    background: color-mix(in srgb, var(--color-media-caption) 14%, transparent);
    color: inherit;
    font-size: var(--font-size-small);
    cursor: pointer;
  }
  .tool.text {
    padding-inline: var(--space-inset-md);
  }
  .tool:hover,
  .nav:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-media-caption) 26%, transparent);
  }
  .tool:focus-visible,
  .nav:focus-visible {
    outline: 2px solid var(--color-media-caption);
    outline-offset: 2px;
  }
  .stage {
    position: relative;
    min-height: 0;
    overflow: hidden;
  }
  /* Fit: centred in the stage, never larger than the window or the image itself. */
  .stage img {
    position: absolute;
    inset: 0 calc(var(--control-height) * 1.25 + var(--space-inset-lg)) var(--space-inset-lg);
    max-width: calc(100% - 2 * (var(--control-height) * 1.25 + var(--space-inset-lg)));
    max-height: calc(100% - var(--space-inset-lg));
    margin: auto;
    cursor: zoom-in;
    background: var(--color-media-backdrop);
  }
  .stage.actual {
    display: grid;
    place-items: start center;
    overflow: auto;
    overscroll-behavior: contain;
  }
  .stage.actual img {
    position: static;
    max-width: none;
    max-height: none;
    cursor: zoom-out;
  }
  .nav {
    position: fixed;
    inset-block-start: 50%;
    translate: 0 -50%;
    width: calc(var(--control-height) * 1.25);
    height: calc(var(--control-height) * 1.25);
  }
  .nav:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .prev {
    inset-inline-start: var(--space-inset-md);
  }
  .prev :global(svg) {
    rotate: 180deg;
  }
  .next {
    inset-inline-end: var(--space-inset-md);
  }
  .caption {
    max-width: 70ch;
    margin: 0 auto;
    padding: var(--space-inset-sm) var(--space-inset-md) var(--space-inset-lg);
    font-size: var(--font-size-small);
    line-height: var(--font-line-height-body);
    text-align: center;
  }
</style>
