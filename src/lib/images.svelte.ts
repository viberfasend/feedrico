import { SvelteSet } from 'svelte/reactivity';

/** Lead Images that failed to load (not cached, too large…). Layouts fall back to text. */
export const failedImages = new SvelteSet<string>();

export function hasLeadImage(key: string | null | undefined): key is string {
  return !!key && !failedImages.has(key);
}
