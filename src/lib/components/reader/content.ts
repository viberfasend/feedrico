// Turns the sanitized Article HTML into something calm to read: every image gets one treatment
// from reader-media.ts, emoji images become text, comic title text becomes a caption, images
// that aren't cached become a card that links to the original, and pictures open in the viewer.
//
// Runs on the DOM after {@html}: the content is already sanitized in the core (no scripts, no
// style attributes), so this only adds classes, captions and the viewer hook.
import { hostOf } from '../../format';
import { intendedSize, isEmojiAlt, presentImage } from '../../reader-media';

export interface ViewerImage {
  src: string;
  alt: string;
  caption: string | null;
  /** Where the image links to, if it sits inside a link. */
  link: string | null;
  /** The original URL, kept by the core in data-remote. */
  original: string | null;
}

export interface ContentOptions {
  /** Opens the image viewer at `index` of `images`. */
  onView: (images: ViewerImage[], index: number) => void;
}

const SAFE_URL = /^https?:\/\//;
const CLASSES = ['img-pixel', 'img-inline', 'img-block', 'img-wide', 'img-cap'];

function attrNumber(img: HTMLImageElement, name: 'width' | 'height'): number | null {
  const value = Number.parseInt(img.getAttribute(name) ?? '', 10);
  return Number.isFinite(value) && value > 0 ? value : null;
}

/** Applies the presentation of `img`: before load from its attributes, after load for real. */
export function classify(img: HTMLImageElement, column: number) {
  const loaded = img.complete && img.naturalWidth > 0;
  const natural = loaded ? { width: img.naturalWidth, height: img.naturalHeight } : null;
  const size = intendedSize(attrNumber(img, 'width'), attrNumber(img, 'height'), natural);
  const look = presentImage(size, column);
  img.classList.remove(...CLASSES);
  img.classList.add(`img-${look.kind}`);
  if (look.wide) img.classList.add('img-wide');
  if (look.capHeight) img.classList.add('img-cap');
  // A wide image uses up to its own width, never more: no upscaling past the original.
  if (look.wide && size) img.style.setProperty('--img-width', `${Math.round(size.width)}px`);
  else img.style.removeProperty('--img-width');
  if (look.kind === 'pixel') img.setAttribute('aria-hidden', 'true');
  if (look.kind === 'block' && !img.closest('a')) makeViewable(img);
  else unmakeViewable(img);
}

function makeViewable(img: HTMLImageElement) {
  img.tabIndex = 0;
  img.setAttribute('role', 'button');
  img.setAttribute('aria-label', img.alt ? `View image: ${img.alt}` : 'View image');
}

function unmakeViewable(img: HTMLImageElement) {
  img.removeAttribute('tabindex');
  if (img.getAttribute('role') === 'button') img.removeAttribute('role');
  if (img.getAttribute('aria-label')?.startsWith('View image')) img.removeAttribute('aria-label');
}

/** The caption a reader sees for `img`: its figure's caption, else a title that adds something. */
function captionOf(img: HTMLImageElement): string | null {
  const figcaption = img.closest('figure')?.querySelector('figcaption');
  if (figcaption?.textContent?.trim()) return figcaption.textContent.trim();
  const title = img.title.trim();
  return title && title !== img.alt.trim() ? title : null;
}

/** Comic title text (xkcd and friends) is part of the joke: show it, don't hide it in a tooltip. */
function addTitleCaption(img: HTMLImageElement) {
  const title = img.title.trim();
  if (!title || title === img.alt.trim()) return;
  if (img.closest('figure')?.querySelector('figcaption')) return;
  if (img.nextElementSibling?.classList.contains('img-caption')) return;
  const caption = document.createElement('span');
  caption.className = 'img-caption';
  caption.textContent = title;
  img.after(caption);
}

/** A card in place of an image that isn't cached (over 5 MB, evicted, not fetched yet). */
export function remoteImageCard(img: HTMLImageElement, remote: string): HTMLElement {
  const card = document.createElement('span');
  card.className = 'remote-image';
  const text = document.createElement('span');
  text.className = 'remote-image-text';
  const title = document.createElement('strong');
  title.textContent = 'Image not saved for offline reading';
  text.append(title);
  if (img.alt.trim()) {
    const alt = document.createElement('span');
    alt.textContent = img.alt.trim();
    text.append(alt);
  }
  card.append(text);
  // Inside a link already: that link stays the target, links can't nest.
  if (!img.closest('a')) {
    const link = document.createElement('a');
    link.className = 'remote-image-action';
    link.href = remote;
    link.textContent = `Open on ${hostOf(remote)}`;
    card.append(link);
  }
  return card;
}

/** Svelte action for the content element. Re-run it (via {#key}) for every Article. */
export function enhanceContent(node: HTMLElement, options: ContentOptions) {
  let current = options;
  const column = () => node.clientWidth || 600;

  const images = () =>
    [...node.querySelectorAll<HTMLImageElement>('img.img-block')].filter((img) => !img.classList.contains('missing'));

  for (const img of node.querySelectorAll<HTMLImageElement>('img')) {
    if (isEmojiAlt(img.alt)) {
      const emoji = document.createElement('span');
      emoji.className = 'emoji';
      emoji.textContent = img.alt.trim();
      img.replaceWith(emoji);
      continue;
    }
    classify(img, column());
    addTitleCaption(img);
  }

  const onLoad = (event: Event) => {
    if (event.target instanceof HTMLImageElement) classify(event.target, column());
  };

  // Images load only from the local cache. One that isn't cached "stays a remote link" (spec):
  // the core keeps its original URL in the inert data-remote attribute. Articles stored before
  // that just hide the broken image.
  const onError = (event: Event) => {
    const img = event.target;
    if (!(img instanceof HTMLImageElement)) return;
    const remote = img.dataset.remote;
    if (img.nextElementSibling?.classList.contains('img-caption')) img.nextElementSibling.remove();
    if (remote && SAFE_URL.test(remote)) img.replaceWith(remoteImageCard(img, remote));
    else img.classList.add('missing');
  };

  const open = (img: HTMLImageElement) => {
    const list = images();
    const index = list.indexOf(img);
    if (index < 0) return;
    current.onView(
      list.map((i) => ({
        src: i.currentSrc || i.src,
        alt: i.alt,
        caption: captionOf(i),
        link: i.closest('a')?.getAttribute('href') ?? null,
        original: i.dataset.remote && SAFE_URL.test(i.dataset.remote) ? i.dataset.remote : null,
      })),
      index,
    );
  };

  const viewable = (target: EventTarget | null) =>
    target instanceof HTMLImageElement && target.getAttribute('role') === 'button' ? target : null;

  const onClick = (event: MouseEvent) => {
    const img = viewable(event.target);
    if (img) open(img);
  };
  const onKeydown = (event: KeyboardEvent) => {
    const img = viewable(event.target);
    if (!img || (event.key !== 'Enter' && event.key !== ' ')) return;
    event.preventDefault();
    event.stopPropagation();
    open(img);
  };

  node.addEventListener('load', onLoad, true);
  node.addEventListener('error', onError, true);
  node.addEventListener('click', onClick);
  node.addEventListener('keydown', onKeydown);
  return {
    update: (next: ContentOptions) => (current = next),
    destroy: () => {
      node.removeEventListener('load', onLoad, true);
      node.removeEventListener('error', onError, true);
      node.removeEventListener('click', onClick);
      node.removeEventListener('keydown', onKeydown);
    },
  };
}
