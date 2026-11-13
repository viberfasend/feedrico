// @vitest-environment happy-dom
// The dev-only component gallery (#gallery): a smoke test that every section renders and that
// the theme/density switchers drive the root attributes. Not part of the production bundle.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import Gallery from './Gallery.svelte';

let mounted: ReturnType<typeof mount> | null = null;

afterEach(() => {
  if (mounted) void unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.density;
});

describe('dev Gallery', () => {
  it('renders a section per primitive', () => {
    mounted = mount(Gallery, { target: document.body });
    flushSync();
    expect(document.querySelector('h1')?.textContent).toBe('Component gallery');
    const sections = [...document.querySelectorAll('main.gallery section h2')].map((h) => h.textContent);
    for (const name of ['Button', 'IconButton', 'Input', 'Menu', 'ContextMenu', 'Dialog', 'Tooltip', 'Toggle', 'Tabs', 'Toast']) {
      expect(sections).toContain(name);
    }
    // A few of the primitives actually render.
    expect(document.querySelectorAll('main.gallery button').length).toBeGreaterThan(5);
    expect(document.querySelector('input#gallery-url')).not.toBeNull();
  });

  it('starts on the OS theme and comfortable density via the root attributes', () => {
    mounted = mount(Gallery, { target: document.body });
    flushSync();
    // happy-dom doesn't implement :checked on <option>, so a select's bind:value can't be
    // driven through the DOM here; the switchers themselves are covered by the Settings tests.
    expect(document.documentElement.dataset.theme, 'system removes the attribute').toBeUndefined();
    expect(document.documentElement.dataset.density).toBe('comfortable');
    const [theme, density] = [...document.querySelectorAll<HTMLSelectElement>('main.gallery select')];
    expect([...theme!.options].map((o) => o.value)).toEqual(['system', 'light', 'dark']);
    expect([...density!.options].map((o) => o.value)).toEqual(['comfortable', 'compact']);
  });
});
