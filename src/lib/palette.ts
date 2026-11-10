// The command palette's fuzzy matcher. Pure and framework-free: a subsequence match scored by
// contiguous runs, word starts and shorter haystacks, case- and accent-insensitive.

export interface Rankable {
  label: string;
  /** Extra text the query may match (e.g. a Feed's site URL). */
  keywords?: string;
}

/** Case- and accent-insensitive form used for both query and haystack. */
function normalize(s: string): string {
  return s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

const WORD_START_BONUS = 8;
const CONTIGUOUS_BONUS = 5;

/** Scores `query` against `text`; `null` when the query is not a subsequence of the text. */
function score(query: string, text: string, original: string): number | null {
  let ti = 0;
  let total = 0;
  let prevMatched = -2;
  for (let qi = 0; qi < query.length; qi++) {
    const ch = query[qi]!;
    const at = text.indexOf(ch, ti);
    if (at === -1) return null;
    const contiguous = at === prevMatched + 1;
    const wordStart =
      at === 0 ||
      /[^a-z0-9]/.test(text[at - 1]!) ||
      (/[a-z]/.test(original[at - 1] ?? '') && /[A-Z]/.test(original[at] ?? ''));
    total += 1 + (contiguous ? CONTIGUOUS_BONUS : 0) + (wordStart ? WORD_START_BONUS : 0);
    prevMatched = at;
    ti = at + 1;
  }
  // A slightly shorter haystack wins when the match itself is equally good.
  return total - text.length * 0.01;
}

/**
 * Ranks `items` by how well `query` matches their label (plus keywords). An empty query returns
 * the items in their original order. Non-matching items are dropped; ties keep the original
 * order, so results are stable while typing.
 */
export function rank<T extends Rankable>(query: string, items: T[]): T[] {
  const q = normalize(query.trim());
  if (!q) return items;
  return items
    .map((item, index) => {
      const hay = normalize(`${item.label} ${item.keywords ?? ''}`);
      const s = score(q, hay, `${item.label} ${item.keywords ?? ''}`);
      return { item, index, s };
    })
    .filter((x): x is { item: T; index: number; s: number } => x.s !== null)
    .sort((a, b) => b.s - a.s || a.index - b.index)
    .map((x) => x.item);
}
