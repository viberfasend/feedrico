// Tests for the command palette's fuzzy matcher: ranking order, accents, empty query, scale.
import { describe, expect, it } from 'vitest';
import { rank } from './palette';

const labels = (items: string[]) => items.map((label) => ({ label }));
const ranked = (query: string, items: string[]) => rank(query, labels(items)).map((i) => i.label);

describe('palette rank()', () => {
  it('returns the original order for an empty query', () => {
    const items = labels(['Zebra', 'Apple', 'Mango']);
    expect(rank('', items)).toEqual(items);
    expect(rank('   ', items)).toEqual(items);
  });

  it('matches a subsequence, not just a prefix: "tch" finds "Tech Crunch"', () => {
    expect(rank('tch', labels(['Tech Crunch', 'Patch notes']))[0]!.label).toBe('Tech Crunch');
  });

  it('prefers contiguous runs and word starts over scattered matches', () => {
    expect(ranked('abc', ['abc', 'aXbXc'])[0]).toBe('abc');
    expect(ranked('fb', ['Foo Bar', 'frab'])[0]).toBe('Foo Bar');
  });

  it('is case-insensitive', () => {
    expect(ranked('TECH', ['tech crunch', 'other'])[0]).toBe('tech crunch');
  });

  it('ignores accents', () => {
    expect(ranked('resume', ['Résumé', 'other'])[0]).toBe('Résumé');
  });

  it('drops items that do not match at all', () => {
    expect(ranked('zzz', ['Tech Crunch', 'Patch notes'])).toEqual([]);
  });

  it('prefers contiguous matches at equal length and keeps original order on ties', () => {
    expect(ranked('ab', ['axb', 'ab'])).toEqual(['ab', 'axb']);
    const tied = rank('x', labels(['x one', 'x two']));
    expect(tied.map((i) => i.label)).toEqual(['x one', 'x two']);
  });

  it('stays instant with 300 Feeds', () => {
    const items = labels(
      Array.from({ length: 300 }, (_, i) => `Feed number ${i} about topic ${i % 17}`),
    );
    const result = rank('topic 7', items);
    expect(result.length).toBeGreaterThan(0);
    expect(result.length).toBeLessThanOrEqual(50);
    expect(result[0]!.label).toContain('topic 7');
  });
});
