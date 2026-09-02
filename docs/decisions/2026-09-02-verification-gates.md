# CodeScene-scoped commit and pull-request verification gates

Date: 2026-09-02. Status: adopted. Supersedes the current gate definitions in the 2026-08-27 verification entry-point and CI-layout records.

## Decision

`scripts/verify.sh` remains the single entry point. Its fast gate runs formatting, linting, all-target builds, rustdoc, the full test suite with at least 90% line coverage, and CodeScene score 10 for every scorable Rust file changed by the commit; local pre-commit runs use the staged file set, while push CI uses the files changed by `HEAD`. `scripts/verify.sh --full` is required for every pull request, runs the same compiler/test/coverage checks, adds `cargo deny --locked check`, and requires CodeScene score 10 for every scorable Rust file changed from the pull-request base through `HEAD`. The base defaults to `origin/staging` and can be supplied by argument or environment. Missing CodeScene credentials fail every gate rather than skipping analysis. Whole-tree CodeScene remains available to establish or audit the baseline. GitHub Actions keeps required-check contexts `verify` for branch pushes and `staging` pull requests and `verify-full` for `main` pull requests.

## Rationale

CodeScene previously ran only when `main` was involved, so ordinary work could merge into `staging` without code-health feedback and defer large refactors until promotion. Checking the commit delta provides immediate local and push feedback; checking the base-relative PR delta prevents a sequence of individually acceptable commits from hiding a regression in the merge candidate. A score-10 whole-tree baseline makes changed-file enforcement sound without repeatedly analyzing unchanged files. Coverage, lint, build, docs, and tests remain identical across the fast and full gates so the only full-only work is pull-request-wide CodeScene scope and dependency policy.

## Revisit when

CodeScene adds a native threshold or changed-file command that can replace JSON score evaluation, the repository adds another scorable language, or measured gate duration requires safe parallelization without weakening per-commit feedback.
