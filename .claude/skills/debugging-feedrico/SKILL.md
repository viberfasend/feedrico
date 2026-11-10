---
name: debugging-feedrico
description: How to run, observe and debug the Feedrico desktop app (Tauri 2 + Rust core + Svelte 5) on this machine, and how to lock a fixed bug in with a regression test. Use when the user reports something broken in the running app, when the dev app misbehaves after a change, or before fixing any UI or Library bug in this repo.
---

# Debugging Feedrico

Feedrico has three layers. Find out which one is wrong **before** changing code:

| Layer | Where | Typical symptom |
|---|---|---|
| Rust core | `crates/feedrico-core` | wrong data, panics in the log, Feeds marked broken |
| Tauri shell | `src-tauri` | commands fail, media not served, lock or startup problems |
| Svelte UI | `src/` | placeholders that never fill, layout overflow, stale views |

Measure, don't guess. Every bug so far looked like one layer and was another (empty Gallery
tiles looked like an image problem; the images were never requested).

## Run the app

```sh
PATH=/snap/bin:$PATH npm run tauri dev     # run in the background, keep the output file
```

- `PATH=/snap/bin:$PATH` is required: a stale Rust 1.72 toolchain comes first on `PATH`.
- The dev app **rebuilds and restarts on every Rust change** and hot-reloads Svelte changes.
  Warn the user before editing Rust while they test.
- Wait for readiness: poll the output until it shows ``Running `…/target/debug/feedrico` `` or an
  error. Don't sleep blindly.
- Is the app on the latest build? Compare `ls -la --time-style=+%T target/debug/feedrico` with
  `date`, and look for `Info File … changed. Rebuilding` / `error[` in the output. A failed
  rebuild leaves the **old** binary running.
- Restarting: stop the background task, then kill the leftover process by an **anchored**
  pattern, e.g. `kill $(pgrep -f '^/home/andreas/dev/altviberssommer/target/debug/feedrico')` or
  `kill $(pgrep -f '^node scripts/serve-site.mjs')`. Unanchored `pkill -f <pattern>` also matches
  the shell running the command (its command line contains the pattern) and kills it.

## See what the UI is doing

- **Webview console goes to the dev log.** `console.warn(...)` in Svelte code appears in the
  `tauri dev` output as `[vite] (client) [console.warn] …`. Tag temporary probes with a unique
  marker (e.g. `[debug-xyz]`) and `grep -a` for it.
- Log the decisive state, not just "here": e.g. `{ total, width, columns, first: app.rowAt(0) }`
  revealed that Gallery tiles had no data at all.
- Rust side: `eprintln!("[debug-xyz] …")` in a command or protocol handler shows up in the
  same output.
- Remove every probe before committing (`grep -rn "debug-xyz" src src-tauri`).
- If the user can't see something, ask them to switch to the view, act, then say "done", and
  read the log.

### Look at it yourself (mock IPC in a browser)

Agents can't screenshot the Tauri window (GNOME Wayland). Unit tests and `svelte-check` don't
see layout. So **look at every visible UI change before calling it done**:

1. With the dev server running, open `http://localhost:1420/` in Chrome (Claude in Chrome).
2. Run [`mock-ipc.js`](mock-ipc.js) in the page. It fakes the Tauri IPC with canned data and
   remounts the real `<App/>`. Extend its `handlers` for the commands your change uses.
3. Screenshot every Layout (List, Cards, Magazine, Gallery) and both themes. Resize down to the
   window's `minWidth` (880 px, `tauri.conf.json`): fixed-width panes (sidebar 260 px, list
   column) are where controls overflow.

Chrome is not WebKitGTK. If the mock looks right but the app doesn't, suspect an engine
difference (see "Known traps") or dev-server state (next section).

### Measure performance

`npx tauri build --no-bundle && npm run perf` runs the Rust query benchmarks and the real app in
perf mode (`FEEDRICO_PERF_LIBRARY`, see `src-tauri/src/perf.rs`, `src/lib/perf.ts`) against a
generated 300 Feed / 50k Article Library and prints a Markdown report. CI runs it on `main`.

## UI glitch that a restart fixes

If the app looks broken but **a restart fixes it**, it's dev-server state, not app code. Don't
patch components for it. Before restarting, collect evidence. After a restart it's gone.

- Grep the `tauri dev` output for `optimized dependencies changed`, `new dependencies
  optimized`, `Outdated Optimize Dep`, `504`, `Failed to resolve`, `hmr update`, `page reload`.
- Note the time and what else ran: other agents (Vibe, Claude worktrees), `npm run
  check|test|build|tokens` in this checkout (each one rewrites `src/lib/tokens/generated/*.css`
  under the running server), `git pull` or a branch switch (changes the lockfile, so deps get
  re-optimized).
- `ls -la --time-style=+%T node_modules/.vite/deps src/lib/tokens/generated` shows when the
  dependency cache and the token CSS were last rewritten.
- Agent worktrees live in `.claude/worktrees/` **inside** the project root. `vite.config.ts` keeps
  the dev server from scanning (`optimizeDeps.entries`) or watching (`server.watch.ignored`)
  them, guarded by `src/lib/dev-server.test.ts`. Keep those guards if you touch the config.

## Inspect the user's Library (read-only)

- Machine-local recent list: `~/.config/de.andi1984.feedrico/recent-libraries.json`
  (first entry is the open Library). The user's Library lives in `~/Dokumente/rssfeed/`.
- Query it **read-only** while the app runs (WAL makes this safe):

  ```sh
  SQLITE=/home/andreas/Android/Sdk/platform-tools/sqlite3
  $SQLITE "file:$LIB/library.db?mode=ro" "select count(*), count(lead_image) from article;"
  ```

- Useful checks: `media` table vs. files in `media/` (`file media/<key>`), `article_media.hash`
  (NULL = not cached), `feed.last_error`, `PRAGMA user_version` vs. `library.json`.
- Never write to the user's Library by hand. Never delete `.lock` by hand: a lock from this
  machine whose process is gone is taken over automatically; foreign locks must ask the user.

## Known traps

- **Svelte 5 `{@const}` is lazy.** An unused `{@const _ = x}` never subscribes to `x`. Make the
  accessor itself reactive (e.g. `rowAt()` reads `pagesVersion`).
- **Async races around list resets.** Bump the epoch before awaiting, discard stale results,
  and clear loading markers in `finally`, or a discarded request blocks its page forever.
- **WebKitGTK and `loading="lazy"`** inside the virtualized scroller: images never load and no
  error fires. Virtualization already limits images to visible rows.
- **Flex items with `<input>`** need `min-width: 0` on the input's flex parent, or the input's
  intrinsic width overflows narrow columns.
- **Adding a control to a fixed-width row** (sidebar footer, list header) silently pushes the
  last controls out of view. The pane clips, so nothing errors. Check at `minWidth` with the
  mock, and wrap or move rarely used actions into a menu.
- **Multi-byte text**: never slice Rust strings by byte offsets from user content; use
  `char_indices`.
- **One problem can hide another** on the attention screen (stale lock *and* pending upgrade).

## Lock the fix in (TDD)

1. Reproduce the bug as a failing test first, at the layer where it lives:
   - Rust core: unit test next to the code (`cargo test -p feedrico-core`); live network checks
     go in `crates/feedrico-core/tests/live.rs` (`-- --ignored`).
   - UI state: `src/lib/state.svelte.test.ts` with the mocked `api` and `deferred()` promises to
     replay exact IPC orderings. Rune tests need `// @vitest-environment happy-dom`.
   - Component source rules: raw-import guards like `src/lib/components/layouts/layouts.test.ts`.
2. **Prove it's red against the old code**: swap in the pre-fix file with
   `git show <commit-before-fix>:<path>`, run the test, see it fail, then restore.
3. Fix, see it green, and name the bug in a comment above the test.
4. Run everything before committing:

   ```sh
   PATH=/snap/bin:$PATH cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
   PATH=/snap/bin:$PATH cargo test --workspace
   npm run check && npm test
   ```

5. Commit locally; push only at the end of a batch (Linux CI runs on billed Blacksmith runners).
