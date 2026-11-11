import { describe, expect, it } from 'vitest';
import { checkPair, contrastRatio, parseHex } from './contrast.mjs';

describe('contrastRatio', () => {
  it('is 21 for black on white', () => {
    expect(contrastRatio('#000', '#fff')).toBeCloseTo(21, 5);
  });
  it('is 1 for identical colours', () => {
    expect(contrastRatio('#E89C26', '#e89c26')).toBeCloseTo(1, 5);
  });
  it('is symmetric', () => {
    expect(contrastRatio('#6A5C49', '#FBF7EF')).toBeCloseTo(contrastRatio('#FBF7EF', '#6A5C49'), 10);
  });
  it('matches a known reference value', () => {
    // #767676 on white is the classic 4.54:1 AA threshold grey.
    expect(contrastRatio('#767676', '#FFFFFF')).toBeCloseTo(4.54, 2);
  });
  it('rejects non-hex input', () => {
    expect(() => parseHex('rgb(0,0,0)')).toThrow();
  });
});

describe('checkPair', () => {
  it('passes a pair that meets the minimum', () => {
    expect(checkPair('#000000', '#FFFFFF', 4.5)).toBeNull();
    expect(checkPair('#AB6510', '#FBF7EF', 3)).toBeNull(); // focus ring on base, light theme
  });
  it('fails a deliberately bad non-text pair (WCAG 1.4.11 needs 3:1)', () => {
    // #D6C7AE (the old light border.default) on #FBF7EF (surface.base) is only ~1.5:1.
    expect(checkPair('#D6C7AE', '#FBF7EF', 3)).toMatch(/needs 3:1/);
  });
  it('fails a text pair just under AA', () => {
    // #767676 on white is 4.54:1 (passes); #777777 is 4.48:1 (fails AA text).
    expect(checkPair('#777777', '#FFFFFF', 4.5)).toMatch(/needs 4.5:1/);
  });
});
