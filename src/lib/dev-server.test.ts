// Guards the dev server against other checkouts inside the repo folder.
//
// Bug (Layout icons gone until the next restart): agent worktrees live in `.claude/worktrees/`
// inside the project root. Vite's dependency scan globs every `**/*.html` under the root and
// its watcher sees every file, so a cold start crawled other agents' half-finished code (with
// dependencies this checkout doesn't have) and their edits fired events in our dev server.
//
// Bug (no hot reload inside a worktree): the first fix ignored `**/.claude/**`, which also
// matched every file of a dev server started *inside* a worktree, so it never saw a change.
import { describe, expect, it } from 'vitest';
import config, { isAgentCheckout } from '../../vite.config';

const root = new URL('../../', import.meta.url).pathname;

describe('dev server', () => {
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
