/** Number of generated tile colours (`--color-feed-tile-1` … `-6`). */
export const FEED_TILES = 6;

/** The tile colour of a Feed, 1-based; stable for the Feed's lifetime. */
export function feedTile(feedId: number): number {
  return (((feedId % FEED_TILES) + FEED_TILES) % FEED_TILES) + 1;
}

/** The first letter or digit of a Feed title, upper-cased; `#` if it has none. */
export function feedInitial(title: string): string {
  const match = title.match(/[\p{L}\p{N}]/u);
  return match ? match[0].toLocaleUpperCase() : '#';
}
