// Guards the dev server against other checkouts inside the repo folder.
//
// Bug (Layout icons gone until the next restart): agent worktrees live in `.claude/worktrees/`
// inside the project root. Vite's dependency scan globs every `**/*.html` under the root and
// its watcher sees every file, so a cold start crawled other agents' half-finished code (with
// dependencies this checkout doesn't have) and their edits fired events in our dev server.
//
// Bug (no hot reload inside a worktree): the first fix ignored `**/.claude/**`, which also
// matched every file of a dev server started *inside* a worktree, so it never saw a change.
//
// Bug (Magazine, Cards and Gallery squeezed into the 380 px list column until App.svelte
// changed): component CSS was a separate virtual module. When the webview asked for it before
// the component's JS was compiled, vite-plugin-svelte logged "failed to load virtual css
// module", Vite served the raw .svelte source as CSS and cached it. The garbled first rule
// (`.shell.grid-layout`) was dropped. In dev, CSS now ships inside the component's JS.
import { createServer } from 'vite';
import { describe, expect, it } from 'vitest';
import viteConfig, { isAgentCheckout } from '../../vite.config';

const root = new URL('../../', import.meta.url).pathname;
const config = viteConfig({ command: 'serve', mode: 'development' });

describe('dev server', () => {
  it('ships component CSS inside the component, not as a separate module', async () => {
    const server = await createServer({
      root,
      configFile: `${root}vite.config.ts`,
      server: { middlewareMode: true, hmr: false, ws: false },
      logLevel: 'silent',
    });
    try {
      const result = await server.transformRequest('/src/App.svelte');
      expect(result?.code).toContain('.shell.grid-layout');
      expect(result?.code).not.toContain('type=style');
    } finally {
      await server.close();
    }
  }, 30_000);

  it('scans only our own entry for dependencies', () => {
    expect(config.optimizeDeps?.entries).toEqual(['index.html']);
  });

  it('ignores the agent worktrees inside this checkout', () => {
    expect(config.server?.watch?.ignored).toContain(isAgentCheckout);
    expect(isAgentCheckout(`${root}.claude/worktrees/agent-x/src/App.svelte`)).toBe(true);
    expect(isAgentCheckout(`${root}.claude/worktrees/agent-x/index.html`)).toBe(true);
  });

  it('still watches its own files, even when this checkout is itself a worktree', () => {
    expect(isAgentCheckout(`${root}src/App.svelte`)).toBe(false);
    expect(isAgentCheckout(`${root}index.html`)).toBe(false);
  });
});
