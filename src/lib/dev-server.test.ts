// Guards the dev server against other checkouts inside the repo folder.
//
// Bug (Layout icons gone until the next restart): agent worktrees live in `.claude/worktrees/`
// inside the project root. Vite's dependency scan globs every `**/*.html` under the root and
// its watcher sees every file, so a cold start crawled other agents' half-finished code (with
// dependencies this checkout doesn't have) and their edits fired events in our dev server.
import { describe, expect, it } from 'vitest';
import config from '../../vite.config';

describe('dev server', () => {
  it('scans only our own entry for dependencies', () => {
    expect(config.optimizeDeps?.entries).toEqual(['index.html']);
  });

  it('does not watch agent worktrees', () => {
    expect(config.server?.watch?.ignored).toContain('**/.claude/**');
  });
});
