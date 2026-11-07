<script lang="ts">
  /** The Altweibersommer motif: drifting spider-silk threads catching late-summer light. */
  let { size = 160 }: { size?: number } = $props();
</script>

<svg
  class="gossamer"
  width={size}
  height={size * 0.6}
  viewBox="0 0 200 120"
  fill="none"
  aria-hidden="true"
>
  <path d="M4 92 C 48 40, 92 112, 140 54 S 188 28, 196 36" />
  <path d="M10 104 C 60 70, 104 118, 150 76 S 186 58, 194 62" />
  <path d="M22 70 C 64 26, 110 82, 152 30" />
  <circle cx="140" cy="54" r="2.5" />
  <circle cx="150" cy="76" r="1.8" />
  <circle cx="64" cy="52" r="1.5" />
</svg>

<style>
  .gossamer path {
    stroke: var(--color-motif-gossamer);
    stroke-width: 1.2;
    stroke-linecap: round;
  }
  .gossamer circle {
    fill: var(--color-unread-indicator);
  }
</style>
