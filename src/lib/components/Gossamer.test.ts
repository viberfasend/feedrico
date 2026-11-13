// @vitest-environment happy-dom
// The gossamer motif is decorative branding: it must never be announced or reachable by keyboard.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import Gossamer from './Gossamer.svelte';

let mounted: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

describe('Gossamer', () => {
  it('renders the motif aria-hidden at the requested size', () => {
    mounted = mount(Gossamer, { target: document.body, props: { size: 220 } });
    flushSync();
    const svg = document.body.querySelector('svg.gossamer')!;
    expect(svg.getAttribute('aria-hidden')).toBe('true');
    expect(svg.getAttribute('width')).toBe('220');
    expect(svg.getAttribute('height')).toBe(String(220 * 0.6));
    expect(svg.getAttribute('tabindex')).toBeNull();
  });

  it('defaults to 160 px wide', () => {
    mounted = mount(Gossamer, { target: document.body });
    flushSync();
    expect(document.body.querySelector('svg.gossamer')!.getAttribute('width')).toBe('160');
  });
});
