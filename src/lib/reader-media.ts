// Rules for presenting the images inside an Article in the reader. Feed HTML carries every kind
// of image: tracking pixels, emoji, share buttons, photos, screenshots, comic strips, tall
// infographics. Each gets one simple, predictable treatment, decided from its size alone.
//
// Pure functions, so every scenario is unit-tested (reader-media.test.ts).

/** Up to this size (both sides, CSS px) an image is part of the text: an icon, a badge, a button. */
export const INLINE_MAX = 32;
/** Images this small are tracking pixels or spacers and are never shown. */
export const PIXEL_MAX = 2;
/** Wider than tall by this much counts as a panorama or strip, which may use the full pane width. */
export const WIDE_RATIO = 2;
/** Taller than wide by more than this is meant to be scrolled (comics, infographics): no height cap. */
export const TALL_RATIO = 2;

export type ImageKind =
  /** Tracking pixel or spacer: hidden. */
  | 'pixel'
  /** Sits in the line of text at its own size. */
  | 'inline'
  /** Its own block, centred, at most the text column wide. */
  | 'block';

export interface ImagePresentation {
  kind: ImageKind;
  /** Block images only: a wide image larger than the column may use the whole pane width. */
  wide: boolean;
  /** Block images only: capped to the pane height so a whole photo fits on screen. */
  capHeight: boolean;
}

export interface Size {
  width: number;
  height: number;
}

/**
 * The size the author meant: width/height attributes win (a 512 px icon marked width="24" is a
 * 24 px icon), one attribute keeps the natural aspect ratio, otherwise the natural size.
 */
export function intendedSize(attrWidth: number | null, attrHeight: number | null, natural: Size | null): Size | null {
  const w = attrWidth && attrWidth > 0 ? attrWidth : null;
  const h = attrHeight && attrHeight > 0 ? attrHeight : null;
  if (w && h) return { width: w, height: h };
  if (natural && natural.width > 0 && natural.height > 0) {
    if (w) return { width: w, height: (w * natural.height) / natural.width };
    if (h) return { width: (h * natural.width) / natural.height, height: h };
    return natural;
  }
  return null;
}

/** How to show an image of `size` (see intendedSize) in a text column `column` px wide. */
export function presentImage(size: Size | null, column: number): ImagePresentation {
  if (!size) return { kind: 'block', wide: false, capHeight: true };
  const { width, height } = size;
  if (width <= PIXEL_MAX && height <= PIXEL_MAX) {
    return { kind: 'pixel', wide: false, capHeight: false };
  }
  if (width <= INLINE_MAX && height <= INLINE_MAX) {
    return { kind: 'inline', wide: false, capHeight: false };
  }
  const wide = width / height >= WIDE_RATIO && width > column;
  const tall = height / width > TALL_RATIO;
  return { kind: 'block', wide, capHeight: !tall };
}

/** WordPress and others put emoji in as images whose alt is the emoji itself: show the text. */
export function isEmojiAlt(alt: string | null | undefined): boolean {
  if (!alt) return false;
  const text = alt.trim();
  return text.length > 0 && text.length <= 16 && /^(?:\p{Extended_Pictographic}|\p{Regional_Indicator}|[‍️\u{1f3fb}-\u{1f3ff}])+$/u.test(text);
}

/** The media key in a cached-image URL (`media://localhost/<key>`, `http://media.localhost/<key>`). */
export function mediaKeyOf(src: string | null | undefined): string | null {
  if (!src) return null;
  const match = /^(?:media:\/\/localhost|https?:\/\/media\.localhost)\/([^/?#]+)/.exec(src);
  return match?.[1] ?? null;
}

/**
 * Whether the reader should put the Lead Image above the content: only when the content has no
 * picture of its own. Summary-only news feeds, photo enclosures and podcasts then still show
 * what the card showed, and a WordPress post never shows its featured image twice, even when
 * the feed's thumbnail is a resized copy under another URL (`photo-150x150.jpg`).
 */
export function showLeadImage(contentHtml: string, leadImage: string | null): boolean {
  return leadImage !== null && !contentHtml.includes(leadImage) && !contentHasPicture(contentHtml);
}

/** Any image in `html` that would show as a picture: not a pixel, icon or emoji (by attributes). */
export function contentHasPicture(html: string): boolean {
  for (const [tag] of html.matchAll(/<img\b[^>]*>/gi)) {
    const attr = (name: string) => new RegExp(`\\s${name}\\s*=\\s*["']?([^"'\\s>]+)`, 'i').exec(tag)?.[1] ?? null;
    if (isEmojiAlt(decodeEntities(attr('alt') ?? ''))) continue;
    const width = Number(attr('width')) || null;
    const height = Number(attr('height')) || null;
    const size = intendedSize(width, height, null);
    if (size && presentImage(size, Infinity).kind !== 'block') continue;
    return true;
  }
  return false;
}

function decodeEntities(text: string): string {
  return text.replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCodePoint(parseInt(h, 16))).replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(Number(d)));
}

/** Minutes to read the text of `html` at 230 words a minute; null when there is no text. */
export function readingMinutes(html: string): number | null {
  const text = html
    .replace(/<(script|style)[^>]*>[\s\S]*?<\/\1>/gi, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&[a-z#0-9]+;/gi, ' ');
  const words = text.split(/\s+/).filter((w) => /\p{L}|\p{N}/u.test(w)).length;
  if (words === 0) return null;
  return Math.max(1, Math.round(words / 230));
}

/** Whether `html` has any visible content: text, an image or an embed card. */
export function hasContent(html: string): boolean {
  return /<img\b/i.test(html) || html.replace(/<[^>]+>/g, '').replace(/&nbsp;/g, ' ').trim().length > 0;
}

export type EnclosureKind = 'audio' | 'video' | 'image' | 'file';

export function enclosureKind(mimeType: string | null, url: string): EnclosureKind {
  const type = (mimeType ?? '').toLowerCase();
  if (type.startsWith('audio/')) return 'audio';
  if (type.startsWith('video/')) return 'video';
  if (type.startsWith('image/')) return 'image';
  const ext = /\.([a-z0-9]{2,4})(?:$|[?#])/i.exec(url)?.[1]?.toLowerCase();
  if (ext && ['mp3', 'm4a', 'aac', 'ogg', 'oga', 'opus', 'wav', 'flac'].includes(ext)) return 'audio';
  if (ext && ['mp4', 'm4v', 'mov', 'webm', 'mkv'].includes(ext)) return 'video';
  if (ext && ['jpg', 'jpeg', 'png', 'gif', 'webp', 'avif'].includes(ext)) return 'image';
  return 'file';
}

/** A readable name for an enclosure: the decoded file name without query, else the host. */
export function enclosureName(url: string): string {
  try {
    const parsed = new URL(url);
    const last = parsed.pathname.split('/').filter(Boolean).pop();
    if (last) return decodeURIComponent(last);
    return parsed.hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

/** "MP3", "JPEG", "PDF" from a MIME type or file name, for the attachment's subtitle. */
export function formatLabel(mimeType: string | null, url: string): string | null {
  const sub = mimeType?.split('/')[1]?.split(';')[0]?.toLowerCase();
  const fromMime: Record<string, string> = { mpeg: 'MP3', mp4: 'MP4', 'x-m4a': 'M4A', jpeg: 'JPEG', pdf: 'PDF' };
  if (sub) return fromMime[sub] ?? sub.replace(/^x-/, '').toUpperCase();
  const ext = /\.([a-z0-9]{2,4})(?:$|[?#])/i.exec(url)?.[1];
  return ext ? ext.toUpperCase() : null;
}
