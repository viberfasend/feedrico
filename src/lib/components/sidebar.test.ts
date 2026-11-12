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

describe('Sidebar tree', () => {
  // Bug (found while checking #61): a long Feed title pushed its row wider than the sidebar.
  // The lists are grids with an `auto` track, whose items never shrink below their content, so
  // the title didn't ellipsize: the sidebar scrolled sideways and the broken-Feed badge and
  // unread count were pushed out of sight. The track must be allowed to shrink.
  it('lets rows shrink to the sidebar width so long titles ellipsize', () => {
    const ul = /\n {2}ul \{([^}]*)\}/.exec(source)?.[1] ?? '';
    expect(ul).toMatch(/display:\s*grid/);
    expect(ul).toMatch(/grid-template-columns:\s*minmax\(0,\s*1fr\)/);
  });
});
