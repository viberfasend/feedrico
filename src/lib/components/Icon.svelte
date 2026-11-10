<script lang="ts" module>
  // Hand-drawn 20px stroke icons; no icon font, no network.
  const paths = {
    refresh: 'M16 10a6 6 0 1 1-1.76-4.24M16 4v3.5h-3.5',
    star: 'M10 3.2l2.06 4.18 4.6.67-3.33 3.25.79 4.58L10 13.72 5.88 15.88l.79-4.58L3.34 8.05l4.6-.67L10 3.2z',
    check: 'M4.5 10.5l3.5 3.5 7.5-8',
    circle: 'M10 4.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z',
    external: 'M11.5 4.5h4v4M15.5 4.5l-6.5 6.5M13.5 11.5v3.5a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1h3.5',
    import: 'M10 3.5v9M6.5 9L10 12.5 13.5 9M4 14.5v1.5a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1v-1.5',
    export: 'M10 12.5v-9M6.5 7L10 3.5 13.5 7M4 14.5v1.5a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1v-1.5',
    search: 'M9 4a5 5 0 1 0 0 10A5 5 0 0 0 9 4zM12.7 12.7L16 16',
    sort: 'M7 4v12M4.5 6.5L7 4l2.5 2.5M13 16V4M10.5 13.5L13 16l2.5-2.5',
    inbox: 'M3.5 11l2-6h9l2 6M3.5 11v4a1 1 0 0 0 1 1h11a1 1 0 0 0 1-1v-4M3.5 11h4l1 2h3l1-2h4',
    sun: 'M10 6.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7zM10 2.5v1.5M10 16v1.5M2.5 10H4M16 10h1.5M4.7 4.7l1 1M14.3 14.3l1 1M4.7 15.3l1-1M14.3 5.7l1-1',
    folder: 'M3.5 6a1 1 0 0 1 1-1h3.5l1.5 1.5h6a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1V6z',
    chevron: 'M8 6l4 4-4 4',
    warning: 'M10 4l6.5 11.5h-13L10 4zM10 9v3M10 13.8v.2',
    library: 'M4.5 4v12M8 4v12M11.5 4.5l3.5 11',
    attachment: 'M14.5 9.5l-5 5a3 3 0 0 1-4.24-4.24l5.5-5.5a2 2 0 0 1 2.83 2.83l-5.5 5.5a1 1 0 0 1-1.42-1.42L11.5 6.5',
    today: 'M4.5 5.5h11v10h-11zM4.5 8.5h11M7.5 3.5v3M12.5 3.5v3',
    list: 'M4 5.5h12M4 10h12M4 14.5h12',
    cards: 'M3.5 4h5.5v5.5H3.5zM11 4h5.5v5.5H11zM3.5 11.5h5.5V16H3.5zM11 11.5h5.5V16H11z',
    magazine: 'M3.5 4h7v12h-7zM12.5 4.5h4M12.5 8h4M12.5 11.5h4M12.5 15h4',
    gallery: 'M3.5 3.5h3.6v3.6H3.5zM8.2 3.5h3.6v3.6H8.2zM12.9 3.5h3.6v3.6h-3.6zM3.5 8.2h3.6v3.6H3.5zM8.2 8.2h3.6v3.6H8.2zM12.9 8.2h3.6v3.6h-3.6zM3.5 12.9h3.6v3.6H3.5zM8.2 12.9h3.6v3.6H8.2zM12.9 12.9h3.6v3.6h-3.6z',
    plus: 'M10 4.5v11M4.5 10h11',
    close: 'M5.5 5.5l9 9M14.5 5.5l-9 9',
    more: 'M4.5 10h1M9.5 10h1M14.5 10h1',
    update: 'M10 4.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11zM10 13V7.5M7.8 9.7L10 7.5l2.2 2.2',
  } as const;
  export type IconName = keyof typeof paths;
</script>

<script lang="ts">
  let {
    name,
    size = 18,
    filled = false,
    label,
  }: { name: IconName; size?: number; filled?: boolean; label?: string } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 20 20"
  fill={filled ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width="1.5"
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
>
  <path d={paths[name]} />
</svg>
