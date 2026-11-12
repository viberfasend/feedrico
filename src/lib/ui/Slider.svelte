<script lang="ts">
  // A range input drawn the same in every engine (WebKitGTK's native slider ignores the theme).
  // `oninput` fires while dragging, for live previews; `onchange` once the value is let go.
  let {
    value,
    min,
    max,
    step = 1,
    label,
    oninput,
    onchange,
  }: {
    value: number;
    min: number;
    max: number;
    step?: number;
    /** Accessible name. */
    label: string;
    oninput?: (value: number) => void;
    onchange: (event: Event) => void;
  } = $props();

  const fill = $derived(`${((value - min) / (max - min)) * 100}%`);
</script>

<input
  class="slider"
  type="range"
  {min}
  {max}
  {step}
  {value}
  aria-label={label}
  style:--fill={fill}
  oninput={(e) => oninput?.(Number((e.currentTarget as HTMLInputElement).value))}
  {onchange}
/>

<style>
  .slider {
    appearance: none;
    -webkit-appearance: none;
    width: 100%;
    height: var(--slider-thumb-size);
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  .slider::-webkit-slider-runnable-track {
    height: var(--slider-track-height);
    border-radius: var(--radius-pill);
    background: linear-gradient(
      to right,
      var(--color-accent-default) var(--fill),
      var(--color-border-subtle) var(--fill)
    );
  }
  .slider::-moz-range-track {
    height: var(--slider-track-height);
    border-radius: var(--radius-pill);
    background: var(--color-border-subtle);
  }
  .slider::-moz-range-progress {
    height: var(--slider-track-height);
    border-radius: var(--radius-pill);
    background: var(--color-accent-default);
  }
  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: var(--slider-thumb-size);
    height: var(--slider-thumb-size);
    margin-top: calc((var(--slider-track-height) - var(--slider-thumb-size)) / 2);
    border: 2px solid var(--color-accent-default);
    border-radius: var(--radius-pill);
    background: var(--color-surface-raised);
    box-shadow: var(--shadow-raised);
    transition: transform var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .slider::-moz-range-thumb {
    width: var(--slider-thumb-size);
    height: var(--slider-thumb-size);
    border: 2px solid var(--color-accent-default);
    border-radius: var(--radius-pill);
    background: var(--color-surface-raised);
  }
  .slider:active::-webkit-slider-thumb {
    transform: scale(1.15);
  }
  .slider:focus-visible {
    outline: none;
  }
  .slider:focus-visible::-webkit-slider-thumb {
    outline: var(--focus-ring-width) solid var(--color-border-focus);
    outline-offset: var(--focus-ring-offset);
  }
  .slider:focus-visible::-moz-range-thumb {
    outline: var(--focus-ring-width) solid var(--color-border-focus);
    outline-offset: var(--focus-ring-offset);
  }
</style>
