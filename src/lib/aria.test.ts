// Screen-reader labels for the virtualized Article list (issue #32): every rendered option is
// announced with its position and its state in words, never as a bare icon or number.
import { describe, expect, it } from 'vitest';
import type { ArticleRow } from './api';
import { articleOptionLabel } from './aria';
import { shortDate } from './format';

const row = (over: Partial<ArticleRow> = {}): ArticleRow => ({
  id: 1,
  feedId: 1,
  feedTitle: 'Rust Blog',
  title: 'A post about feeds',
  excerpt: '',
  articleDate: 1_700_000_000,
  isRead: false,
  isStarred: false,
  leadImage: null,
  favicon: null,
  ...over,
});

describe('articleOptionLabel', () => {
  it('reads title, Feed, Article Date and unread state', () => {
    const label = articleOptionLabel(row());
    expect(label).toBe(`A post about feeds, Rust Blog, ${shortDate(1_700_000_000)}, unread`);
  });

  it('names Starred and Read state in words', () => {
    expect(articleOptionLabel(row({ isStarred: true }))).toContain('starred, unread');
    expect(articleOptionLabel(row({ isRead: true }))).toContain('read');
    expect(articleOptionLabel(row({ isRead: true, isStarred: true }))).toContain('starred, read');
  });

  it('never says "seen" or "opened" — Read is the only word', () => {
    const label = articleOptionLabel(row({ isRead: true }));
    expect(label).not.toMatch(/seen|opened|viewed/i);
  });
});
