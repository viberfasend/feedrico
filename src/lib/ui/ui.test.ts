// @vitest-environment happy-dom
// Contract tests for the component library in src/lib/ui. Each test pins the accessibility
// contract a wrapper must keep: labels, roles, keyboard paths, focus behaviour.
import { createRawSnippet, flushSync, mount, type Component, type ComponentProps } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Button from './Button.svelte';
import IconButton from './IconButton.svelte';
import Input from './Input.svelte';
import Toggle from './Toggle.svelte';
import Toast from './Toast.svelte';
import ContextMenuFixture from './fixtures/ContextMenuFixture.svelte';
import DialogFixture from './fixtures/DialogFixture.svelte';
import MenuFixture from './fixtures/MenuFixture.svelte';
import TabsFixture from './fixtures/TabsFixture.svelte';

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const text = (html: string) => createRawSnippet(() => ({ render: () => html }));

afterEach(() => {
  document.body.innerHTML = '';
});

function mountToBody<C extends Component<any>>(component: C, props: ComponentProps<C>) {
  const c = mount(component, { target: document.body, props });
  flushSync();
  return c;
}

describe('Button', () => {
  it('renders children and fires onclick', () => {
    const onclick = vi.fn();
    mountToBody(Button, { onclick, children: text('<span>Save</span>') });
    const btn = document.body.querySelector('button')!;
    expect(btn.textContent).toContain('Save');
    btn.click();
    flushSync();
    expect(onclick).toHaveBeenCalledTimes(1);
  });

  it('does not fire onclick when disabled', () => {
    const onclick = vi.fn();
    mountToBody(Button, { onclick, disabled: true, children: text('Save') });
    const btn = document.body.querySelector('button')!;
    expect(btn).toHaveProperty('disabled', true);
    btn.click();
    expect(onclick).not.toHaveBeenCalled();
  });
});

describe('IconButton', () => {
  // An icon-only button must be named: the label becomes aria-label.
  it('exposes the label as aria-label and renders an icon', () => {
    mountToBody(IconButton, { label: 'Check for updates', icon: 'refresh' });
    const btn = document.body.querySelector('button')!;
    expect(btn.getAttribute('aria-label')).toBe('Check for updates');
    expect(btn.querySelector('svg')).not.toBeNull();
  });
});

describe('Input', () => {
  it('renders a labelled text input with the current value', () => {
    mountToBody(Input, { value: 'https://example.com/feed', placeholder: 'Feed address', id: 'url' });
    const input = document.body.querySelector('input')!;
    expect(input.value).toBe('https://example.com/feed');
    expect(input.placeholder).toBe('Feed address');
    expect(input.id).toBe('url');
  });
});

describe('Toggle', () => {
  // A toggle button exposes its state as aria-pressed.
  it('exposes aria-pressed and flips on click', () => {
    mountToBody(Toggle, { pressed: true, label: 'Compact' });
    const btn = document.body.querySelector('button')!;
    expect(btn.getAttribute('aria-pressed')).toBe('true');
    btn.click();
    flushSync();
    expect(btn.getAttribute('aria-pressed')).toBe('false');
  });
});

describe('Toast', () => {
  it('renders the message and runs the action', () => {
    const run = vi.fn();
    const onDismiss = vi.fn();
    mountToBody(Toast, { message: 'Feedrico 0.2.0 is available', action: { label: 'Open', run }, onDismiss });
    expect(document.body.textContent).toContain('Feedrico 0.2.0 is available');
    const actionBtn = [...document.body.querySelectorAll('button')].find((b) => b.textContent === 'Open')!;
    actionBtn.click();
    expect(run).toHaveBeenCalledTimes(1);
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});

describe('Dialog', () => {
  it('opens from the trigger, closes on Escape and returns focus to the trigger', async () => {
    mountToBody(DialogFixture, {});
    const trigger = document.getElementById('open-dialog') as HTMLButtonElement;
    trigger.focus();
    trigger.click();
    flushSync();
    await settle();
    const dialog = document.querySelector('[role="dialog"]');
    expect(dialog, 'dialog is shown').not.toBeNull();

    dialog!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    flushSync();
    await settle();
    expect(document.querySelector('[role="dialog"]'), 'Escape closes the dialog').toBeNull();
    expect(document.activeElement, 'focus returns to the trigger').toBe(trigger);
  });
});

describe('Menu', () => {
  it('opens from the keyboard and items are reachable with arrow keys', async () => {
    mountToBody(MenuFixture, {});
    const trigger = document.body.querySelector('button')!;
    trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    flushSync();
    await settle();
    const menu = document.querySelector('[role="menu"]');
    expect(menu, 'Enter opens the menu').not.toBeNull();
    const items = [...document.querySelectorAll('[role="menuitem"]')];
    expect(items.map((i) => i.textContent)).toEqual(['First', 'Second']);
    // Bug (More menu rendered full-width below the window): with a `child` snippet, bits-ui
    // hands over `wrapperProps` for the positioned wrapper. Dropping them left menus unplaced.
    expect(menu!.closest('[data-bits-floating-content-wrapper]'), 'the menu is positioned').not.toBeNull();

    menu!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
    flushSync();
    await settle();
    expect(document.activeElement?.getAttribute('role')).toBe('menuitem');
  });
  // An icon-only trigger (the sidebar's "More" button) needs a name for screen readers.
  it('names its trigger', () => {
    mountToBody(MenuFixture, {});
    expect(document.body.querySelector('button')?.getAttribute('aria-label')).toBe('More actions');
  });
});

describe('ContextMenu', () => {
  it('opens on right-click and on the ContextMenu key', async () => {
    mountToBody(ContextMenuFixture, {});
    const target = document.getElementById('ctx-target') as HTMLButtonElement;

    target.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 10, clientY: 10 }));
    flushSync();
    await settle();
    let menu = document.querySelector('[role="menu"]');
    expect(menu, 'right-click opens the menu').not.toBeNull();
    expect(menu!.closest('[data-bits-floating-content-wrapper]'), 'the menu is positioned').not.toBeNull();
    expect([...document.querySelectorAll('[role="menuitem"]')].map((i) => i.textContent)).toEqual(['Rename', 'Delete']);
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    flushSync();
    await settle();

    target.dispatchEvent(new KeyboardEvent('keydown', { key: 'ContextMenu', bubbles: true, cancelable: true }));
    flushSync();
    await settle();
    menu = document.querySelector('[role="menu"]');
    expect(menu, 'the ContextMenu key opens the menu too').not.toBeNull();
  });
});

describe('Tabs', () => {
  it('renders a tab list and switches panels on click', async () => {
    mountToBody(TabsFixture, {});
    const tabs = [...document.querySelectorAll('[role="tab"]')];
    expect(tabs.map((t) => t.textContent)).toEqual(['One', 'Two']);
    expect(tabs[0]!.getAttribute('aria-selected')).toBe('true');
    expect(document.body.textContent).toContain('Panel one');

    (tabs[1] as HTMLElement).click();
    flushSync();
    await settle();
    expect(tabs[1]!.getAttribute('aria-selected')).toBe('true');
    expect(document.body.textContent).toContain('Panel two');
  });
});
