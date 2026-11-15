// @vitest-environment happy-dom
// The Assistant bridge's SearchQueryResult narrowing (#122): the shell already validated the
// answer, so the UI only needs to trust the shape or reject it.
import { describe, expect, it } from 'vitest';
import { parseSearchQueryResult } from './assistant';

describe('parseSearchQueryResult', () => {
  it('accepts a validated result', () => {
    expect(
      parseSearchQueryResult({ query: 'title:"wal"*', since: 1, until: 2, feed: 5, folder: null, dropped: ['Nope'] }),
    ).toEqual({ query: 'title:"wal"*', since: 1, until: 2, feed: 5, folder: null, dropped: ['Nope'] });
  });

  it('turns missing or wrongly typed fields into null and rejects a missing query', () => {
    expect(parseSearchQueryResult({ query: 'x' })).toEqual({
      query: 'x',
      since: null,
      until: null,
      feed: null,
      folder: null,
      dropped: [],
    });
    for (const bad of [null, undefined, 'x', 5, [], {}, { query: '' }, { query: '  ' }, { query: 5 }]) {
      expect(parseSearchQueryResult(bad)).toBeNull();
    }
  });
});
