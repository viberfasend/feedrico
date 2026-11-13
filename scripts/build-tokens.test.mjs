// The token build: primitives resolve but are never emitted, themes land on the right
// selectors (including the OS-dark media query), densities on data-density, the TypeScript
// accessor maps every token to its CSS variable, and the WCAG 2.2 AA contrast gate passes.
// Runs the real script in a child process so a contrast failure exits non-zero there instead
// of killing the test runner.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const root = new URL('../', import.meta.url).pathname;
const generated = (file) => readFileSync(new URL(`../src/lib/tokens/generated/${file}`, import.meta.url), 'utf8');

describe('build-tokens.mjs', () => {
  it(
    'builds all token outputs and passes the contrast check',
    () => {
      const out = execFileSync('node', ['scripts/build-tokens.mjs'], {
        cwd: root,
        encoding: 'utf8',
        timeout: 120_000,
      });
      expect(out).toMatch(/Tokens built; \d+ contrast pairs pass WCAG 2\.2 AA in light and dark\./);

      const base = generated('base.css');
      expect(base).toMatch(/:root \{/);
      // Semantic base + component tokens land in base.css…
      expect(base).toContain('--font-family-ui:');
      expect(base).toContain('--article-list-width:');
      // …densities override the component tokens per data-density…
      expect(generated('density-comfortable.css')).toContain('--article-list-row-height:');
      // …and colours live in the theme files.
      expect(generated('theme-light.css')).toContain('--color-surface-base:');
      // Primitives are only included for resolution; they are never emitted.
      for (const file of ['base.css', 'theme-light.css', 'theme-dark.css', 'density-comfortable.css', 'density-compact.css']) {
        expect(generated(file), `${file} emits no primitive tokens`).not.toMatch(/--palette-/);
      }

      const light = generated('theme-light.css');
      expect(light).toMatch(/:root,\n\[data-theme="light"\] \{/);
      const dark = generated('theme-dark.css');
      expect(dark).toMatch(/\[data-theme="dark"\] \{/);
      expect(dark).toMatch(/@media \(prefers-color-scheme: dark\)/);
      expect(dark).toMatch(/:root:not\(\[data-theme="light"\]\)/);

      expect(generated('density-comfortable.css')).toMatch(/:root,\n\[data-density="comfortable"\] \{/);
      expect(generated('density-compact.css')).toMatch(/\[data-density="compact"\] \{/);

      const tokens = generated('tokens.ts');
      expect(tokens).toMatch(/export const t = /);
      expect(tokens).toMatch(/export type Tokens = typeof t;/);
      // The accessor references the CSS variables, not the raw values.
      expect(tokens).toContain('"var(--color-surface-base)"');
      expect(tokens).not.toContain('#FBF7EF');
    },
    180_000,
  );
});
