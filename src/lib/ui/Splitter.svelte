<script lang="ts">
  // Window splitter (WAI-ARIA APG): a focusable separator that sizes the pane it controls.
  // Arrows move the separator by STEP px (Shift: LARGE_STEP), Home/End go to the limits,
  // double-click resets, drag uses pointer capture. Enter fires `ontoggle` when given.
  import { stepForKey } from '../panes';

  let {
    value,
    min,
    max,
    label,
    controls,
    onchange,
    onreset,
    ontoggle,
    inverted = false,
    style = '',
  }: {
    /** Current width of the controlled pane in px. */
    value: number;
    min: number;
    max: number;
    label: string;
    /** Id of the pane this splitter sizes. */
    controls: string;
    onchange: (px: number) => void;
    onreset: () => void;
    ontoggle?: () => void;
    /** The pane grows when the separator moves left (a pane on the right edge). */
    inverted?: boolean;
    /** Positioning, set by the parent. */
    style?: string;
  } = $props();

  let dragging = $state(false);
  let startX = 0;
  let startValue = 0;

  function onkeydown(event: KeyboardEvent) {
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.key === 'Enter' && ontoggle) {
      event.preventDefault();
      event.stopPropagation();
      ontoggle();
      return;
    }
    const next = stepForKey(event.key, event.shiftKey, value, min, max, inverted);
    if (next === null) return;
    // These keys would otherwise reach the global shortcuts (arrow keys move the selection).
    event.preventDefault();
    event.stopPropagation();
    if (next !== value) onchange(next);
  }

  function onpointerdown(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const el = event.currentTarget as HTMLElement;
    el.setPointerCapture(event.pointerId);
    el.focus({ preventScroll: true });
    dragging = true;
    startX = event.clientX;
    startValue = value;
  }

  function onpointermove(event: PointerEvent) {
    if (!dragging) return;
    const delta = (event.clientX - startX) * (inverted ? -1 : 1);
    const next = Math.min(Math.max(startValue + delta, min), max);
    if (Math.round(next) !== Math.round(value)) onchange(next);
  }

  function endDrag() {
    dragging = false;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  aria-label={label}
  aria-controls={controls}
  aria-valuenow={Math.round(value)}
  aria-valuemin={Math.round(min)}
  aria-valuemax={Math.round(max)}
  tabindex="0"
  {style}
  {onkeydown}
  {onpointerdown}
  {onpointermove}
  onpointerup={endDrag}
  onpointercancel={endDrag}
  onlostpointercapture={endDrag}
  ondblclick={onreset}
></div>

<style>
  .splitter {
    position: absolute;
    inset-block: 0;
    width: var(--splitter-hit-size);
    cursor: col-resize;
    touch-action: none;
    user-select: none;
  }
  .splitter::after {
    content: '';
    position: absolute;
    inset-block: 0;
    inset-inline-start: calc((var(--splitter-hit-size) - var(--splitter-line-width)) / 2);
    width: var(--splitter-line-width);
    background: transparent;
  }
  .splitter:hover::after,
  .splitter.dragging::after {
    background: var(--color-accent-default);
  }
  .splitter:focus-visible {
    outline: none;
  }
  .splitter:focus-visible::after {
    background: var(--color-border-focus);
  }
</style>
