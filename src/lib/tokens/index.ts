export { t, type Tokens } from './generated/tokens';

export type Theme = 'system' | 'light' | 'dark';
export type Density = 'comfortable' | 'compact';

/**
 * Themes and densities are token sets selected by root attributes, so switching is a single
 * attribute write: no re-render, no stylesheet swap. "system" removes the attribute and lets the
 * prefers-color-scheme media query decide.
 */
export function applyTheme(theme: Theme, root: HTMLElement = document.documentElement): void {
  if (theme === 'system') root.removeAttribute('data-theme');
  else root.dataset.theme = theme;
}

export function applyDensity(density: Density, root: HTMLElement = document.documentElement): void {
  root.dataset.density = density;
}
