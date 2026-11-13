# Automated PR review: linted workflows, a tailored review comment, coverage reports, CodeQL

Pull requests get automatic review help instead of relying on reviewers remembering the
repo's rules. `.github/workflows/review.yml` (GitHub-hosted, pull requests only) lints the
workflow files with actionlint, labels the PR by changed area, posts one review comment
tailored to the diff, and reports test coverage (vitest v8 for `src/` + `scripts/`,
cargo llvm-cov for `feedrico-core`) as a PR comment plus lcov artifacts.
`.github/workflows/codeql.yml` runs CodeQL for JavaScript/TypeScript and Rust on pull
requests. The comment makes the rules visible at the moment they matter: Library format,
locking or startup changes must walk the version scenario matrix A–E with The Carrier and
The Host, UI changes must consider the WCAG 2.2 AA audit, perf-relevant changes must cite
the performance budgets, and risky added lines (`dbg!`, `console.log`, `.unwrap()`,
`@ts-ignore`, …) are flagged. This helps The Archivist (Henrik) by keeping the performance
budgets in front of every perf-touching PR, The Carrier (Jonas) and The Host (Ute) by
forcing the A–E walk on Library changes, The Offline Reader (Paul) via CodeQL on the
local-only code paths, and the whole accessibility spectrum in `docs/personas.md` via the
WCAG checklist on UI PRs.

## Consequences

- Every new job runs on GitHub-hosted `ubuntu-latest` and the new workflows have no `push`
  trigger: Linux CI on billed Blacksmith runners runs on pull requests only, never again on
  `main` pushes. Coverage is deliberately on the free runner even though the Rust build is
  slow, and llvm-cov is scoped to `feedrico-core` so no webview dependencies are needed.
- The review comment is one comment per PR, updated in place, so it never spams the
  timeline; the coverage comment does the same. Both need `pull-requests: write`.
- CodeQL uploads need `security-events: write`, which same-repo PRs have; fork PRs run
  read-only and skip the upload. No scheduled CodeQL: schedules would rerun on main.
- The review script is heuristic (diff keywords, patch scanning), not a replacement for
  human review — it exists to make the repo's own checklists impossible to forget.
