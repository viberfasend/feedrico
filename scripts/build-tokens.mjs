// Builds design tokens (W3C DTCG JSON in tokens/) into CSS custom properties and a typed
// TypeScript accessor, then fails the build if any declared colour pair misses WCAG 2.2 AA.
//
// Output (generated, not committed): src/lib/tokens/generated/
//   base.css               theme- and density-independent semantic + component tokens
//   theme-light.css        :root and [data-theme="light"]
//   theme-dark.css         [data-theme="dark"], and the OS dark preference when no theme is set
//   density-*.css          [data-density="…"]
//   tokens.ts              t.color.surface.base → "var(--color-surface-base)"

import { readFile } from 'node:fs/promises';
import StyleDictionary from 'style-dictionary';
import { fileHeader, formattedVariables } from 'style-dictionary/utils';
import { contrastRatio } from './contrast.mjs';

const OUT = 'src/lib/tokens/generated/';
const PRIMITIVES = ['tokens/primitive/**/*.json'];

StyleDictionary.registerFormat({
  name: 'css/feedrico-scoped',
  format: async ({ dictionary, file, options }) => {
    const vars = formattedVariables({
      format: 'css',
      dictionary,
      outputReferences: false,
      usesDtcg: options.usesDtcg,
    });
    let css = (await fileHeader({ file })) + `${options.selector} {\n${vars}\n}\n`;
    if (options.mediaQuery) {
      const nested = vars.replace(/^/gm, '  ');
      css += `\n@media ${options.mediaQuery} {\n  ${options.mediaSelector} {\n${nested}\n  }\n}\n`;
    }
    return css;
  },
});

StyleDictionary.registerFormat({
  name: 'typescript/feedrico-accessor',
  format: async ({ dictionary, file }) => {
    const tree = {};
    for (const token of dictionary.allTokens) {
      let node = tree;
      token.path.slice(0, -1).forEach((key) => (node = node[key] ??= {}));
      node[token.path.at(-1)] = `var(--${token.name})`;
    }
    return (
      (await fileHeader({ file })) +
      `/** Every semantic and component token as a CSS variable reference. */\n` +
      `export const t = ${JSON.stringify(tree, null, 2)} as const;\n\n` +
      `export type Tokens = typeof t;\n`
    );
  },
});

const onlySource = (token) => token.isSource;

/** One Style Dictionary run: primitives are included for resolution but never emitted. */
function build({ source, files, include = [] }) {
  return new StyleDictionary({
    include: [...PRIMITIVES, ...include],
    source,
    log: { verbosity: 'default', warnings: 'error' },
    platforms: {
      css: {
        transformGroup: 'css',
        buildPath: OUT,
        files: files.map((f) => ({ filter: onlySource, ...f })),
      },
    },
  });
}

const runs = [
  {
    source: ['tokens/semantic/base.json', 'tokens/component/base.json'],
    files: [{ destination: 'base.css', format: 'css/feedrico-scoped', options: { selector: ':root' } }],
  },
  {
    source: ['tokens/semantic/theme.light.json'],
    files: [
      {
        destination: 'theme-light.css',
        format: 'css/feedrico-scoped',
        options: { selector: ':root,\n[data-theme="light"]' },
      },
    ],
  },
  {
    source: ['tokens/semantic/theme.dark.json'],
    files: [
      {
        destination: 'theme-dark.css',
        format: 'css/feedrico-scoped',
        options: {
          selector: '[data-theme="dark"]',
          mediaQuery: '(prefers-color-scheme: dark)',
          mediaSelector: ':root:not([data-theme="light"])',
        },
      },
    ],
  },
  {
    source: ['tokens/component/density.comfortable.json'],
    include: ['tokens/semantic/base.json'],
    files: [
      {
        destination: 'density-comfortable.css',
        format: 'css/feedrico-scoped',
        options: { selector: ':root,\n[data-density="comfortable"]' },
      },
    ],
  },
  {
    source: ['tokens/component/density.compact.json'],
    include: ['tokens/semantic/base.json'],
    files: [
      {
        destination: 'density-compact.css',
        format: 'css/feedrico-scoped',
        options: { selector: '[data-density="compact"]' },
      },
    ],
  },
  {
    // Names are identical across themes and densities, so one representative set types them all.
    source: [
      'tokens/semantic/base.json',
      'tokens/semantic/theme.light.json',
      'tokens/component/base.json',
      'tokens/component/density.comfortable.json',
    ],
    files: [{ destination: 'tokens.ts', format: 'typescript/feedrico-accessor' }],
  },
];

for (const run of runs) await build(run).buildAllPlatforms();

// --- Contrast check -------------------------------------------------------------------------

function flatten(node, path = [], out = {}) {
  if (node && typeof node === 'object' && '$value' in node) {
    out[path.join('.')] = node.$value;
    return out;
  }
  for (const [key, child] of Object.entries(node ?? {})) {
    if (!key.startsWith('$')) flatten(child, [...path, key], out);
  }
  return out;
}

const { pairs } = JSON.parse(await readFile('tokens/contrast-pairs.json', 'utf8'));
const failures = [];
for (const theme of ['light', 'dark']) {
  const sd = new StyleDictionary({
    include: PRIMITIVES,
    source: [`tokens/semantic/theme.${theme}.json`],
    log: { verbosity: 'silent' },
    platforms: { css: { transformGroup: 'css' } },
  });
  const colours = flatten(await sd.exportPlatform('css'));
  for (const { fg, bg, min } of pairs) {
    if (!(fg in colours) || !(bg in colours)) {
      failures.push(`${theme}: unknown token in pair ${fg} on ${bg}`);
      continue;
    }
    const ratio = contrastRatio(colours[fg], colours[bg]);
    if (ratio < min) {
      failures.push(`${theme}: ${fg} on ${bg} is ${ratio.toFixed(2)}:1, needs ${min}:1`);
    }
  }
}

if (failures.length) {
  console.error(`\nWCAG contrast check failed:\n  ${failures.join('\n  ')}\n`);
  process.exit(1);
}
console.log(`Tokens built; ${pairs.length * 2} contrast pairs pass WCAG 2.2 AA in light and dark.`);
