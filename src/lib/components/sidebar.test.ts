import { describe, expect, it } from 'vitest';
import source from './Sidebar.svelte?raw';

describe('Sidebar footer', () => {
  // Bug (theme and update buttons unreachable): the footer sits in the fixed 260 px sidebar and
  // clips silently. Each new icon button pushed the last ones under the Article list. Two icon
  // buttons plus the More menu fit next to the Library name; everything else goes in the menu.
  it('keeps at most two icon buttons next to the More menu', () => {
    const footer = source.slice(source.indexOf('<footer'), source.indexOf('</footer>'));
    expect(footer.match(/<IconButton\b/g)?.length ?? 0).toBeLessThanOrEqual(2);
    expect(footer).toMatch(/<Menu\b/);
  });
});
