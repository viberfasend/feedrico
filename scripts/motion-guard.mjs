// Motion guard: every animation and transition duration must come from a duration token
// (var(--motion-duration-*) in CSS, a token read in Svelte transition directives), never a raw
// literal. The single exception is the prefers-reduced-motion override itself, which must set
// durations to ~0.
//
// Used by scripts/motion-guard.test.mjs, which fails the test run on any raw duration in
// src/**/*.svelte or src/**/*.css (generated token CSS excluded).

import { readdir, readFile } from 'node:fs/promises';
import { join, relative } from 'node:path';

/** CSS properties that carry a duration. */
const DURATION_PROP = String.raw`(?:transition|animation|transition-duration|animation-duration)\s*:[^;}]*?\b(\d+(?:\.\d+)?(?:ms|s))\b`;
const CSS_RAW_DURATION = new RegExp(DURATION_PROP, 'g');
/** Svelte `transition:fade={{ duration: 200 }}` style directives in markup. */
const SVELTE_RAW_DURATION = String.raw`\b(?:transition|animate):[A-Za-z]+\s*=\s*\{\{[^}]*?\bduration:\s*(\d+(?:\.\d+)?)\b`;
const SVELTE_RAW_DURATION_RE = new RegExp(SVELTE_RAW_DURATION, 'g');

/** Removes `@media (prefers-reduced-motion: …) { … }` blocks (brace-balanced). */
export function stripReducedMotion(css) {
  let out = '';
  let i = 0;
  while (i < css.length) {
    const m = /@media\s*\(\s*prefers-reduced-motion/.exec(css.slice(i));
    if (!m) {
      out += css.slice(i);
      break;
    }
    const start = i + m.index;
    out += css.slice(i, start);
    const open = css.indexOf('{', start);
    if (open === -1) {
      out += css.slice(start);
      break;
    }
    let depth = 0;
    let j = open;
    for (; j < css.length; j++) {
      if (css[j] === '{') depth++;
      else if (css[j] === '}') {
        depth--;
        if (depth === 0) break;
      }
    }
    i = j + 1;
  }
  return out;
}

/** Extracts the <style> blocks of a Svelte component. */
export function styleBlocks(svelteSource) {
  const blocks = [];
  const re = /<style[^>]*>([\s\S]*?)<\/style>/g;
  let m;
  while ((m = re.exec(svelteSource))) blocks.push(m[1]);
  return blocks;
}

/**
 * Returns the raw durations found in one source file, as `line: duration` strings.
 * `kind` is 'svelte' or 'css'.
 */
export function findRawDurations(source, kind) {
  const found = [];
  const scan = (text, offsetBase) => {
    for (const re of [CSS_RAW_DURATION, ...(kind === 'svelte' ? [SVELTE_RAW_DURATION_RE] : [])]) {
      re.lastIndex = 0;
      let m;
      while ((m = re.exec(text))) {
        const line = text.slice(0, m.index).split('\n').length + offsetBase;
        found.push(`${line}: ${m[1]}`);
      }
    }
  };
  if (kind === 'svelte') {
    let base = 0;
    for (const block of styleBlocks(source)) {
      scan(stripReducedMotion(block), source.slice(0, source.indexOf(block, base)).split('\n').length);
      base = source.indexOf(block, base) + block.length;
    }
    // Markup directives are scanned outside <style> and <script> blocks.
    const markup = source
      .replace(/<style[^>]*>[\s\S]*?<\/style>/g, '')
      .replace(/<script[^>]*>[\s\S]*?<\/script>/g, '');
    scan(markup, 0);
  } else {
    scan(stripReducedMotion(source), 0);
  }
  return found;
}

/** All hand-written Svelte and CSS files under src/, excluding generated token output. */
export async function srcStyleFiles(root = 'src') {
  const files = [];
  async function walk(dir) {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        if (path === 'src/lib/tokens/generated') continue;
        await walk(path);
      } else if (/\.(svelte|css)$/.test(entry.name)) {
        files.push(path);
      }
    }
  }
  await walk(root);
  return files.sort();
}

/** Raw durations across all of src/, as `file:line: duration`. */
export async function scanSrc(root = 'src') {
  const violations = [];
  for (const file of await srcStyleFiles(root)) {
    const source = await readFile(file, 'utf8');
    const kind = file.endsWith('.svelte') ? 'svelte' : 'css';
    for (const hit of findRawDurations(source, kind)) {
      violations.push(`${relative('.', file)}:${hit}`);
    }
  }
  return violations;
}
