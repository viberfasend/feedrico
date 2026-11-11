import { describe, expect, it } from 'vitest';
import { findRawDurations, scanSrc, stripReducedMotion } from './motion-guard.mjs';

describe('motion guard fixtures', () => {
  it('fails on a raw duration in a CSS transition (fixture with 200ms)', () => {
    const css = '.x { transition: opacity 200ms ease-in-out; }';
    expect(findRawDurations(css, 'css')).toEqual(['1: 200ms']);
  });

  it('fails on a raw animation duration and on seconds', () => {
    const css = '.y { animation: spin 0.9s linear infinite; }';
    expect(findRawDurations(css, 'css')).toEqual(['1: 0.9s']);
  });

  it('passes duration tokens and var() references', () => {
    const css = [
      '.a { transition: opacity var(--motion-duration-fast) var(--motion-easing-standard); }',
      '.b { animation: spin var(--motion-duration-spin) linear infinite; }',
      '.c { transition-duration: var(--motion-duration-base); }',
    ].join('\n');
    expect(findRawDurations(css, 'css')).toEqual([]);
  });

  it('passes the prefers-reduced-motion override, which must be near-zero', () => {
    const css = `@media (prefers-reduced-motion: reduce) {
      *, *::before, *::after {
        animation-duration: 0.01ms !important;
        transition-duration: 0.01ms !important;
      }
    }`;
    expect(stripReducedMotion(css)).not.toMatch(/0\.01ms/);
    expect(findRawDurations(css, 'css')).toEqual([]);
  });

  it('fails on a raw duration in a Svelte transition directive', () => {
    const svelte = `<div transition:fade={{ duration: 200 }}></div>`;
    expect(findRawDurations(svelte, 'svelte')).toEqual(['1: 200']);
  });

  it('passes a Svelte transition whose duration comes from a token read', () => {
    const svelte = `<div transition:fly={{ x: 80, duration: tokens.px('--motion-duration-base') }}></div>`;
    expect(findRawDurations(svelte, 'svelte')).toEqual([]);
  });

  it('scans <style> blocks of a component but not its markup text', () => {
    const svelte = `<script>const s = 'transition: 5s';</script>\n<style>.z { transition: color var(--motion-duration-fast); }</style>`;
    expect(findRawDurations(svelte, 'svelte')).toEqual([]);
  });
});

describe('motion guard over src/', () => {
  it('finds no raw animation/transition durations in src/**/*.svelte or src/**/*.css', async () => {
    expect(await scanSrc('src')).toEqual([]);
  });
});
