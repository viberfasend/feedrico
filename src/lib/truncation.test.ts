import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

// Guard for "every ellipsized text shows its full text": each CSS rule with
// `text-overflow: ellipsis` must match markup that carries a `title` attribute.

const root = join(__dirname, 'components');

/** Ellipsized selectors whose full text is already the accessible name. */
const allowlist: Record<string, string> = {
  'CommandPalette.svelte .option-label':
    'The option button names itself from this text, and the palette is keyboard-driven.',
};

function svelteFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? svelteFiles(join(dir, e.name)) : e.name.endsWith('.svelte') ? [join(dir, e.name)] : [],
  );
}

/** Opening tags of the template, skipping `>` inside quotes and `{...}`. */
function openingTags(markup: string): string[] {
  const tags: string[] = [];
  const re = /<([a-zA-Z][\w-]*)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(markup))) {
    let i = m.index;
    let depth = 0;
    let quote = '';
    for (; i < markup.length; i++) {
      const c = markup[i];
      if (quote) {
        if (c === quote && depth === 0) quote = '';
      } else if (c === '{') depth++;
      else if (c === '}') depth--;
      else if ((c === '"' || c === "'") && depth === 0) quote = c;
      else if (c === '>' && depth === 0) break;
    }
    tags.push(markup.slice(m.index, i + 1));
  }
  return tags;
}

/** Last compound of a selector, e.g. `.row.folder .title:hover` -> classes ['title']. */
function target(selector: string) {
  const last = selector
    .trim()
    .split(/\s+|>|\+|~/)
    .filter(Boolean)
    .pop()!
    .replace(/::?[\w-]+(\([^)]*\))?/g, '');
  const tag = /^[a-z][\w-]*/.exec(last)?.[0] ?? '';
  const classes = [...last.matchAll(/\.([\w-]+)/g)].map((c) => c[1]!);
  return { tag, classes };
}

function matches(tagSrc: string, t: { tag: string; classes: string[] }): boolean {
  const name = /^<([\w-]+)/.exec(tagSrc)![1]!;
  if (t.tag && name !== t.tag) return false;
  const cls = /\sclass="([^"]*)"/.exec(tagSrc)?.[1]?.split(/\s+/) ?? [];
  return t.classes.every((c) => cls.includes(c) || new RegExp(`\\sclass:${c}[=\\s}>/]`).test(tagSrc));
}

describe('truncated text', () => {
  const found: string[] = [];
  for (const file of svelteFiles(root)) {
    const src = readFileSync(file, 'utf8');
    const styleAt = src.indexOf('<style');
    if (styleAt < 0) continue;
    const markup = src.slice(0, styleAt).replace(/<script[\s\S]*?<\/script>/g, '');
    const css = src.slice(styleAt);
    const name = relative(root, file);
    const base = name.split('/').pop()!;
    for (const [, sel = ''] of css.matchAll(/([^{}]+)\{[^{}]*text-overflow:\s*ellipsis[^{}]*\}/g)) {
      for (const selector of sel.replace(/\/\*[\s\S]*?\*\//g, '').split(',')) {
        const key = `${base} ${selector.trim().split(/\s+/).pop()}`;
        found.push(key);
        it(`${name}: ${selector.trim()} has its full text in title`, () => {
          const t = target(selector);
          const hits = openingTags(markup).filter((tag) => matches(tag, t));
          expect(hits.length, 'no markup matches this selector').toBeGreaterThan(0);
          if (key in allowlist) return;
          // A descendant selector (`.library span`) is not resolved against its ancestors, so
          // one titled match is enough there; a single compound must hold for every match.
          const titled = (tag: string) => /\stitle=/.test(tag);
          if (selector.trim().includes(' ')) expect(hits.some(titled), 'missing title=').toBe(true);
          else for (const tag of hits) expect(tag, 'missing title=').toMatch(/\stitle=/);
        });
      }
    }
  }
  it('finds ellipsized selectors, and the allowlist has no stale entries', () => {
    expect(found.length).toBeGreaterThan(10);
    for (const key of Object.keys(allowlist)) expect(found).toContain(key);
  });
});
