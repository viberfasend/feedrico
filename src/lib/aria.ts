// Screen-reader labels for the Article list. Only rendered rows exist in the DOM (virtualized),
// so each option carries its position (aria-posinset/aria-setsize) and its state in words.
import type { ArticleRow } from './api';
import { shortDate } from './format';

/** Full spoken label for an Article option: title, Feed, Article Date, Starred/Read state. */
export function articleOptionLabel(article: ArticleRow): string {
  const state = [
    article.isStarred ? 'starred' : null,
    article.isRead ? 'read' : 'unread',
  ]
    .filter(Boolean)
    .join(', ');
  return `${article.title}, ${article.feedTitle}, ${shortDate(article.articleDate)}, ${state}`;
}
