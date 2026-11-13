// @vitest-environment happy-dom
// Empty states (Mira, The Newcomer): every empty screen says what to do next — a title, a plain
// explanation and, where a next step exists, action buttons. The motif stays decorative.
import { createRawSnippet, flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import EmptyState from './EmptyState.svelte';

const text = (html: string) => createRawSnippet(() => ({ render: () => html }));

let mounted: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
});

function render(props: { title: string; children?: ReturnType<typeof text>; actions?: ReturnType<typeof text> }) {
  mounted = mount(EmptyState, { target: document.body, props });
  flushSync();
  return document.body.querySelector('.empty') as HTMLElement;
}

describe('EmptyState', () => {
  it('renders the title, the explanation and the gossamer motif (aria-hidden)', () => {
    const el = render({ title: 'All caught up', children: text('Enjoy the late-summer sun.') });
    expect(el.querySelector('h2')?.textContent).toBe('All caught up');
    expect(el.querySelector('p')?.textContent).toBe('Enjoy the late-summer sun.');
    expect(el.querySelector('svg.gossamer')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('offers a next step through the actions slot when there is one', () => {
    const el = render({
      title: 'Add your first Feed',
      children: text('Paste the address of a website you like to read.'),
      actions: text('<span><button>Add a website or Feed…</button><button>Import a list of Feeds…</button></span>'),
    });
    const actions = el.querySelector('.actions')!;
    expect(actions.textContent).toContain('Add a website or Feed…');
    expect(actions.textContent).toContain('Import a list of Feeds…');
    expect(actions.querySelectorAll('button')).toHaveLength(2);
  });

  it('renders without actions or explanation when only a title is given', () => {
    const el = render({ title: 'Nothing here' });
    expect(el.querySelector('h2')?.textContent).toBe('Nothing here');
    expect(el.querySelector('p')).toBeNull();
    expect(el.querySelector('.actions')).toBeNull();
  });
});
