# Feedrico

A fast, local-only desktop RSS reader: Tauri 2, a Rust core (`crates/feedrico-core`), a thin
shell (`src-tauri`) and a Svelte 5 UI (`src`). All user state lives in one portable Library.

## Read before deciding or building

- [`GLOSSARY.md`](GLOSSARY.md): the words we use (Library, Feed, Article, View, Layout, …) and
  the ones we avoid. Use them in code, UI copy, issues and commits.
- [`docs/spec/v1.md`](docs/spec/v1.md): what v1 does, including the performance budgets.
- [`docs/adr/`](docs/adr/): decisions that are hard to reverse. Don't undo them silently.
- [`docs/personas.md`](docs/personas.md): who we build for. In every design discussion, ADR and
  issue, name the personas a decision helps or hurts. Test against their scenarios, and walk the
  version scenario matrix (A–E) for any change to the Library format, locking or startup.

## Working here

- Debugging, running the dev app, inspecting a Library, and writing regression tests: use the
  `debugging-feedrico` skill (`.claude/skills/debugging-feedrico/`).
- Fix bugs test-first: a failing test that reproduces the bug, proven red against the old code.
- Prefer the simple, predictable rule over clever automation.
- Batch work on one branch; commit locally and push once per batch. Linux CI runs on billed
  Blacksmith runners.
- Rust needs `PATH=/snap/bin:$PATH` on this machine.

## Commit messages

[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/), enforced by the
`commit-msg` hook (installed by `npm install`) and in CI. They drive versioning and release notes.

```
type(scope): subject in imperative mood, lowercase start, no period

Body: why, not what. Wrap at 72.

BREAKING CHANGE: what breaks and what to do      (or `type!:` in the header)
```

- **Types:** `feat` (new capability, minor bump), `fix` (bug fix, patch), `perf`, `refactor`,
  `test`, `docs`, `build`, `ci`, `chore`, `style`, `revert`.
- **Scopes (optional):** `core`, `shell`, `ui`, `tokens`, `library`, `feeds`, `layouts`, `spec`,
  `ci`, `deps`, `release`.
- A change to the Library format is breaking for older apps: mark it `feat(library)!` with a
  `BREAKING CHANGE:` footer and walk the persona version matrix A–E.
- GitHub merges by rebase only, so every commit lands on `main` as written. Keep each commit
  meaningful on its own.

## Checks before committing

```sh
PATH=/snap/bin:$PATH cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
PATH=/snap/bin:$PATH cargo test --workspace
npm run check && npm test
```
