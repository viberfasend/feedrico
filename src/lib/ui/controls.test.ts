// @vitest-environment happy-dom
// Contract tests for the form controls ui.test.ts doesn't cover: Segmented, Select, Slider,
// Tooltip. Same rules as ui.test.ts: accessible name, value flow, keyboard-reachable natives.
import { createRawSnippet, flushSync, mount, type Component, type ComponentProps } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Segmented from './Segmented.svelte';
import Select from './Select.svelte';
import Slider from './Slider.svelte';
import Tooltip from './Tooltip.svelte';

const text = (html: string) => createRawSnippet(() => ({ render: () => html }));

afterEach(() => {
  document.body.innerHTML = '';
});

function mountToBody<C extends Component<any>>(component: C, props: ComponentProps<C>) {
  const c = mount(component, { target: document.body, props });
  flushSync();
  return c;
}

describe('Segmented', () => {
  it('is a named radio group with the current option checked', () => {
    mountToBody(Segmented, {
      value: 'cards',
      options: [
        { value: 'list', label: 'List' },
        { value: 'cards', label: 'Cards' },
      ],
      label: 'Layout',
      onchange: () => {},
    });
    const group = document.body.querySelector('[role="radiogroup"]')!;
    expect(group.getAttribute('aria-label')).toBe('Layout');
    const radios = [...document.body.querySelectorAll('input[type="radio"]')] as HTMLInputElement[];
    expect(radios.map((r) => r.value)).toEqual(['list', 'cards']);
    expect(radios.map((r) => r.checked)).toEqual([false, true]);
  });

  it('reports the chosen option through onchange', () => {
    const onchange = vi.fn();
    mountToBody(Segmented, {
      value: 'list',
      options: [
        { value: 'list', label: 'List' },
        { value: 'cards', label: 'Cards' },
      ],
      label: 'Layout',
      onchange,
    });
    const radios = [...document.body.querySelectorAll('input[type="radio"]')] as HTMLInputElement[];
    radios[1]!.dispatchEvent(new Event('change', { bubbles: true }));
    expect(onchange).toHaveBeenCalledWith('cards');
  });
});

describe('Select', () => {
  it('renders a named native select with the current value', () => {
    mountToBody(Select, {
      value: '30',
      options: [
        { value: '15', label: '15 minutes' },
        { value: '30', label: '30 minutes' },
      ],
      label: 'Refresh every',
      onchange: () => {},
    });
    const select = document.body.querySelector('select') as HTMLSelectElement;
    expect(select.getAttribute('aria-label')).toBe('Refresh every');
    expect(select.value).toBe('30');
    expect([...select.options].map((o) => o.textContent)).toEqual(['15 minutes', '30 minutes']);
  });

  it('reports the new value through onchange', () => {
    const onchange = vi.fn();
    mountToBody(Select, {
      value: '30',
      options: [
        { value: '15', label: '15 minutes' },
        { value: '30', label: '30 minutes' },
      ],
      label: 'Refresh every',
      onchange,
    });
    const select = document.body.querySelector('select') as HTMLSelectElement;
    select.value = '15';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    expect(onchange).toHaveBeenCalledWith('15');
  });
});

describe('Slider', () => {
  it('renders a named range input with its range and value', () => {
    mountToBody(Slider, {
      value: 18,
      min: 14,
      max: 24,
      step: 1,
      label: 'Font size',
      onchange: () => {},
    });
    const input = document.body.querySelector('input[type="range"]') as HTMLInputElement;
    expect(input.getAttribute('aria-label')).toBe('Font size');
    expect(input.min).toBe('14');
    expect(input.max).toBe('24');
    expect(input.step).toBe('1');
    expect(input.value).toBe('18');
  });

  // Settings previews the value while dragging (oninput) and saves on release (onchange).
  it('reports numbers on input and the change event on release', () => {
    const oninput = vi.fn();
    const onchange = vi.fn();
    mountToBody(Slider, { value: 18, min: 14, max: 24, label: 'Font size', oninput, onchange });
    const input = document.body.querySelector('input[type="range"]') as HTMLInputElement;
    input.value = '21';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    expect(oninput).toHaveBeenCalledWith(21);
    input.dispatchEvent(new Event('change', { bubbles: true }));
    expect(onchange).toHaveBeenCalledTimes(1);
  });
});

describe('Tooltip', () => {
  it('wraps its trigger in a named button and keeps the text out of the DOM until shown', () => {
    mountToBody(Tooltip, { text: 'Check GitHub Releases for a newer version', children: text('<span>Hover me</span>') });
    const trigger = document.body.querySelector('button.tooltip-trigger')!;
    expect(trigger.textContent).toContain('Hover me');
    expect(document.body.querySelector('.tooltip')).toBeNull();
  });
});
