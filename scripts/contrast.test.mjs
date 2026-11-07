import { describe, expect, it } from 'vitest';
import { contrastRatio, parseHex } from './contrast.mjs';

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
