// @vitest-environment happy-dom
// Lead Image bookkeeping: Layouts fall back to text for Articles whose image failed to load
// (not cached, over 5 MB, evicted). Clara: a View with few images must look deliberate, and a
// failed image must not keep retrying on every render.
import { flushSync } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { failedImages, hasLeadImage } from './images.svelte';

afterEach(() => {
  failedImages.clear();
});

describe('hasLeadImage', () => {
  it('is false for a missing key', () => {
    expect(hasLeadImage(null)).toBe(false);
    expect(hasLeadImage(undefined)).toBe(false);
    expect(hasLeadImage('')).toBe(false);
  });

  it('is true for a key that has not failed', () => {
    expect(hasLeadImage('a'.repeat(32))).toBe(true);
  });

  it('turns false once the image failed, and true again when the failure is forgotten', () => {
    const key = 'b'.repeat(32);
    expect(hasLeadImage(key)).toBe(true);
    failedImages.add(key);
    expect(hasLeadImage(key)).toBe(false);
    failedImages.delete(key);
    expect(hasLeadImage(key)).toBe(true);
  });

  it('stays reactive: effects re-run when an image fails', () => {
    const key = 'c'.repeat(32);
    const seen: boolean[] = [];
    const stop = $effect.root(() => {
      $effect(() => {
        seen.push(hasLeadImage(key));
      });
    });
    flushSync();
    failedImages.add(key);
    flushSync();
    stop();
    expect(seen).toEqual([true, false]);
  });
});
