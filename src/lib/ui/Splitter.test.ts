// @vitest-environment happy-dom
import { flushSync, mount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Splitter from './Splitter.svelte';

afterEach(() => {
  document.body.innerHTML = '';
});

function setup(props: Partial<{ value: number; inverted: boolean; ontoggle: () => void }> = {}) {
  const onchange = vi.fn();
  const onreset = vi.fn();
  mount(Splitter, {
    target: document.body,
    props: { value: 260, min: 200, max: 420, label: 'Resize sidebar', controls: 'pane-sidebar', onchange, onreset, ...props },
  });
  flushSync();
  const el = document.querySelector('[role="separator"]') as HTMLElement;
  const press = (key: string, init: KeyboardEventInit = {}) => {
    const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
    el.dispatchEvent(event);
    return event;
  };
  return { el, press, onchange, onreset };
}

describe('Splitter', () => {
  it('is a vertical separator with value, bounds, label and controlled pane', () => {
    const { el } = setup();
    expect(el.getAttribute('aria-orientation')).toBe('vertical');
    expect(el.getAttribute('aria-valuenow')).toBe('260');
    expect(el.getAttribute('aria-valuemin')).toBe('200');
    expect(el.getAttribute('aria-valuemax')).toBe('420');
    expect(el.getAttribute('aria-label')).toBe('Resize sidebar');
    expect(el.getAttribute('aria-controls')).toBe('pane-sidebar');
    expect(el.tabIndex).toBe(0);
  });

  it('resizes with arrows and Shift+arrows', () => {
    const { press, onchange } = setup();
    press('ArrowRight');
    expect(onchange).toHaveBeenLastCalledWith(276);
    press('ArrowLeft');
    expect(onchange).toHaveBeenLastCalledWith(244);
    press('ArrowRight', { shiftKey: true });
    expect(onchange).toHaveBeenLastCalledWith(324);
  });

  it('jumps to the limits on Home and End', () => {
    const { press, onchange } = setup();
    press('Home');
    expect(onchange).toHaveBeenLastCalledWith(200);
    press('End');
    expect(onchange).toHaveBeenLastCalledWith(420);
  });

  it('does not report a change at a limit', () => {
    const { press, onchange } = setup({ value: 420 });
    press('ArrowRight');
    expect(onchange).not.toHaveBeenCalled();
  });

  it('keeps resize keys from reaching the global shortcuts', () => {
    const { el, onchange } = setup();
    const outer = vi.fn();
    document.addEventListener('keydown', outer);
    el.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    document.removeEventListener('keydown', outer);
    expect(outer).not.toHaveBeenCalled();
    expect(onchange).toHaveBeenCalled();
  });

  it('toggles on Enter only when asked to', () => {
    const ontoggle = vi.fn();
    expect(setup({ ontoggle }).press('Enter').defaultPrevented).toBe(true);
    expect(ontoggle).toHaveBeenCalledOnce();
    document.body.innerHTML = '';
    expect(setup().press('Enter').defaultPrevented).toBe(false);
  });

  it('grows on ArrowLeft when inverted', () => {
    const { press, onchange } = setup({ value: 300, inverted: true });
    press('ArrowLeft');
    expect(onchange).toHaveBeenLastCalledWith(316);
  });

  it('resets on double-click', () => {
    const { el, onreset } = setup();
    el.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(onreset).toHaveBeenCalledOnce();
  });
});
