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
- Restarting: stop the background task, then `pkill -f target/debug/feedrico`. Never run
  `pkill -f vite` or other broad patterns: they match the shell running the command and kill it.

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
