// Image rules for the reader, one test per real-world scenario found in feeds.
import { describe, expect, it } from 'vitest';
import {
  contentHasPicture,
  enclosureKind,
  enclosureName,
  formatDuration,
  formatLabel,
  hasContent,
  intendedSize,
  isEmojiAlt,
  mediaKeyOf,
  presentImage,
  readingMinutes,
  showLeadImage,
} from './reader-media';

const COLUMN = 600;
const present = (width: number, height: number) => presentImage({ width, height }, COLUMN);

describe('presentImage', () => {
  it('hides tracking pixels and spacers (FeedBurner, newsletters)', () => {
    expect(present(1, 1).kind).toBe('pixel');
    expect(present(2, 1).kind).toBe('pixel');
  });

  it('keeps a hairline divider that is wide, not a pixel', () => {
    expect(present(600, 1).kind).toBe('block');
  });

  it('puts share buttons and icons into the line of text', () => {
    expect(present(24, 24)).toMatchObject({ kind: 'inline' });
    expect(present(16, 16).kind).toBe('inline');
    expect(present(32, 20).kind).toBe('inline');
  });

  it('shows a small photo as its own block at its own size', () => {
    expect(present(320, 200)).toEqual({ kind: 'block', wide: false, capHeight: true });
  });

  it('fits a large landscape photo to the column', () => {
    expect(present(1600, 1067)).toEqual({ kind: 'block', wide: false, capHeight: true });
  });

  it('caps a portrait photo to the pane height so the whole picture fits on screen', () => {
    expect(present(1067, 1600)).toMatchObject({ capHeight: true, wide: false });
  });

  it('never caps a tall comic or infographic: it is meant to be scrolled', () => {
    expect(present(740, 2400)).toMatchObject({ kind: 'block', capHeight: false });
  });

  it('lets a wide comic strip or panorama use the whole pane width', () => {
    expect(present(1800, 520)).toMatchObject({ wide: true });
  });

  it('keeps a wide but small banner inside the column', () => {
    expect(present(400, 100).wide).toBe(false);
  });

  it('treats an image of unknown size as a plain block', () => {
    expect(presentImage(null, COLUMN)).toMatchObject({ kind: 'block', capHeight: true });
  });
});

describe('intendedSize', () => {
  it('lets width/height attributes win over the natural size (retina icons)', () => {
    expect(intendedSize(24, 24, { width: 512, height: 512 })).toEqual({ width: 24, height: 24 });
  });

  it('keeps the natural aspect ratio when only one attribute is set', () => {
    expect(intendedSize(300, null, { width: 1200, height: 600 })).toEqual({ width: 300, height: 150 });
    expect(intendedSize(null, 50, { width: 200, height: 100 })).toEqual({ width: 100, height: 50 });
  });

  it('uses the natural size without attributes, and knows nothing before load', () => {
    expect(intendedSize(null, null, { width: 800, height: 533 })).toEqual({ width: 800, height: 533 });
    expect(intendedSize(null, null, null)).toBeNull();
    expect(intendedSize(1, 1, null)).toEqual({ width: 1, height: 1 });
  });
});

describe('isEmojiAlt', () => {
  it('recognises emoji images by their alt text (WordPress wp-smiley)', () => {
    expect(isEmojiAlt('😀')).toBe(true);
    expect(isEmojiAlt('👍🏽')).toBe(true);
    expect(isEmojiAlt('🇩🇪')).toBe(true);
    expect(isEmojiAlt('👩‍💻')).toBe(true);
    expect(isEmojiAlt('❤️')).toBe(true);
  });

  it('leaves ordinary alt text alone', () => {
    expect(isEmojiAlt('Chart of requests')).toBe(false);
    expect(isEmojiAlt(':)')).toBe(false);
    expect(isEmojiAlt('')).toBe(false);
    expect(isEmojiAlt(null)).toBe(false);
    expect(isEmojiAlt('2024')).toBe(false);
  });
});

describe('Lead Image in the content', () => {
  it('reads the media key from both media URL forms (Linux/macOS and Windows)', () => {
    expect(mediaKeyOf('media://localhost/abc123')).toBe('abc123');
    expect(mediaKeyOf('http://media.localhost/abc123')).toBe('abc123');
    expect(mediaKeyOf('https://example.com/abc123')).toBeNull();
  });

  it('leads a summary-only Article (news, photo enclosure, podcast)', () => {
    expect(showLeadImage('<p>Summary only</p>', 'k1')).toBe(true);
    expect(showLeadImage('', 'k1')).toBe(true);
    expect(showLeadImage('<p>x</p>', null)).toBe(false);
  });

  it('is not repeated when the content shows the same cached image (WordPress featured image)', () => {
    expect(showLeadImage('<p><img src="media://localhost/k1"></p>', 'k1')).toBe(false);
  });

  it('is not repeated when the content has a picture under another URL (resized copy)', () => {
    expect(showLeadImage('<p><img src="media://localhost/k2" width="1024" height="683"></p>', 'k1')).toBe(false);
    expect(showLeadImage('<img src="media://localhost/k2">', 'k1')).toBe(false);
  });

  it('still leads when the content only has icons, pixels and emoji', () => {
    const html =
      '<p>Great <img src="media://localhost/e" alt="&#x1f600;" width="72" height="72"> news ' +
      '<img src="media://localhost/s" width="24" height="24" alt="Share"></p><img src="media://localhost/p" width="1" height="1">';
    expect(contentHasPicture(html)).toBe(false);
    expect(showLeadImage(html, 'k1')).toBe(true);
  });
});

describe('readingMinutes and hasContent', () => {
  it('counts words of the text, not of the markup', () => {
    const words = Array.from({ length: 460 }, () => 'word').join(' ');
    expect(readingMinutes(`<p class="x">${words}</p>`)).toBe(2);
    expect(readingMinutes('<p>Short</p>')).toBe(1);
    expect(readingMinutes('<img src="media://localhost/k">')).toBeNull();
    expect(readingMinutes('')).toBeNull();
  });

  it('knows a title-only Article has nothing to show', () => {
    expect(hasContent('')).toBe(false);
    expect(hasContent('<p> &nbsp; </p>')).toBe(false);
    expect(hasContent('<img src="media://localhost/k">')).toBe(true);
    expect(hasContent('<p>Hi</p>')).toBe(true);
  });
});

describe('enclosures', () => {
  it('knows the kind from the MIME type, then the file extension', () => {
    expect(enclosureKind('audio/mpeg', 'x')).toBe('audio');
    expect(enclosureKind('video/mp4', 'x')).toBe('video');
    expect(enclosureKind('image/jpeg', 'x')).toBe('image');
    expect(enclosureKind(null, 'https://cdn.example/ep.mp3?ref=rss')).toBe('audio');
    expect(enclosureKind('application/pdf', 'https://x.example/a.pdf')).toBe('file');
  });

  it('names an enclosure by its decoded file name, without tracking query', () => {
    expect(enclosureName('https://cdn.example/audio/ep88%20final.mp3?utm=feed')).toBe('ep88 final.mp3');
    expect(enclosureName('https://www.example.com/')).toBe('example.com');
    expect(enclosureName('not a url')).toBe('not a url');
  });

  it('labels the format in plain words', () => {
    expect(formatLabel('audio/mpeg', 'x')).toBe('MP3');
    expect(formatLabel('image/jpeg', 'x')).toBe('JPEG');
    expect(formatLabel('audio/x-m4a', 'x')).toBe('M4A');
    expect(formatLabel(null, 'https://x.example/a.pdf')).toBe('PDF');
    expect(formatLabel(null, 'https://x.example/a')).toBeNull();
  });

  it('formats play time as m:ss, and h:mm:ss over an hour', () => {
    expect(formatDuration(754)).toBe('12:34');
    expect(formatDuration(3725)).toBe('1:02:05');
    expect(formatDuration(5)).toBe('0:05');
    expect(formatDuration(null)).toBeNull();
    expect(formatDuration(0)).toBeNull();
  });
});
