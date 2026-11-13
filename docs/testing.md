# Testing and PR automation

How to run the tests locally, and what runs on every pull request.

## Locally

```sh
# UI + tokens + scripts: type-check (svelte-check, axe-core) and vitest
npm run check
npm test

# Rust core + Tauri shell (Rust needs PATH=/snap/bin:$PATH on this machine)
PATH=/snap/bin:$PATH cargo fmt --all
PATH=/snap/bin:$PATH cargo clippy --workspace --all-targets -- -D warnings
PATH=/snap/bin:$PATH cargo test --workspace
```

All four before every commit (see `CLAUDE.md`).

## Coverage locally

```sh
# UI (src/) and scripts (scripts/): vitest v8 coverage -> coverage/lcov.info,
# coverage/coverage-summary.json and a text table. `coverage/` is generated output.
npm run test:coverage

# Rust core: one-time setup, then
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
cargo llvm-cov -p feedrico-core --lcov --output-path lcov-rust.info
```

`feedrico-core` is covered on its own because it is pure Rust (bundled SQLite) and needs no
webview libraries, unlike the Tauri shell. That also keeps the PR job on cheap runners.

## On pull requests

| Workflow | Trigger | Runners | What it does |
|---|---|---|---|
| `ci.yml` | `pull_request`, `workflow_dispatch` | Blacksmith (billed) for Linux; GitHub-hosted for Windows/macOS | `npm run check`, `npm test`, `npm run build`; `cargo fmt/clippy/test`; Tauri build on 3 OSes |
| `commits.yml` | `pull_request` + `push` to main | GitHub-hosted | commitlint on the PR's commits |
| `review.yml` | `pull_request`, `workflow_dispatch` | GitHub-hosted | see below |
| `codeql.yml` | `pull_request`, `workflow_dispatch` | GitHub-hosted | CodeQL for JavaScript/TypeScript and Rust, uploads to code scanning |
| `perf.yml` | weekly cron + dispatch | Blacksmith (billed) | performance budgets report (never gates) |
| `release.yml` | `v*` tags | mixed | draft release + packages |

`review.yml` jobs, all on `ubuntu-latest`:

- **actionlint** — lints every `.github/workflows/*.yml` with rhysd/actionlint, so a broken
  workflow fails on the PR that changes it. `.github/actionlint.yaml` declares the
  self-hosted Blacksmith runner labels so the existing workflows lint clean.
- **labeler** — applies one label per changed top-level area (`.github/labeler.yml`:
  `src/` → `ui`, `crates/` + `src-tauri/` → `core`, `.github/` → `ci`, `docs/` → `docs`,
  `site/` → `site`).
- **review** — posts ONE comment on the PR (updated on every push) with the changed
  top-level areas, which `ci.yml` jobs the diff triggers, and a review checklist tailored
  to the diff: Library format/locking/startup changes must walk the persona version
  matrix A–E (`docs/personas.md`); UI changes must consider the WCAG 2.2 AA audit
  (`docs/accessibility.md`); perf-relevant changes must cite the budgets in
  `docs/spec/v1.md`. It also flags risky added lines (`dbg!`, `console.log`, `println!`,
  `TODO`/`FIXME`, `.unwrap()`/`.expect()` in Rust, `@ts-ignore` in TS).
- **coverage** — runs `npm run test:coverage` and `cargo llvm-cov -p feedrico-core`,
  uploads `coverage/lcov.info`, `coverage/coverage-summary.json` and `lcov-rust.info` as
  the `coverage-report` artifact, and posts ONE comment with the totals for `src` +
  `scripts` and for the core, plus the least-covered files.

## Why everything new is GitHub-hosted and PR-only

Linux CI runs on billed Blacksmith runners, so the rule is: **CI runs on pull requests
only, never again on `main` pushes** (GitHub merges by rebase, so a push to main is the PR
that just passed — re-running would double the billed minutes). `review.yml` and
`codeql.yml` therefore have no `push` trigger at all, and every job in them runs on
GitHub-hosted `ubuntu-latest`, which is free for a public repository — the same precedent
as `commits.yml`.

Two deliberate choices inside that rule:

- The **coverage job is slow but free**: a full Rust build on a GitHub-hosted runner takes
  minutes, but costs nothing, while putting it on Blacksmith would add billed minutes to
  every PR. Scoping llvm-cov to `feedrico-core` keeps it free of the webview dependencies
  and the Tauri shell build.
- **CodeQL runs as one job** for both languages on `ubuntu-latest` (free for public repos,
  uploads need `security-events: write`, which same-repo PRs have; fork PRs skip the
  upload). No `schedule` trigger — scheduled scans would rerun on main, which we don't do.

We did not add vitest JUnit output with `EnricoMi/publish-unit-test-result-action` to the
`ci.yml` frontend job: failed tests already fail that job with full logs in the PR, and
annotating them would mean either touching the billed job or a second test run in the free
coverage job. Simple and predictable wins; revisit if PR test annotations become useful.
