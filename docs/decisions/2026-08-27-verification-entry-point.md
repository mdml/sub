# Verification entry point and gates

Date: 2026-08-27. Status: adopted.

## Decision

`scripts/verify.sh` is the single verification entry point. Without arguments it runs the per-commit gate: `cargo fmt --check`, `cargo clippy` with warnings as errors, `cargo build --all-targets`, `cargo doc` with warnings as errors, and the test suite under `cargo llvm-cov` failing below 90 % line coverage. With `--full` it adds `cargo deny check` and the CodeScene step (`scripts/codescene.sh`, code health 10 for every eligible file). `just verify` and `just verify-full` are aliases; CI calls the script directly. See `docs/verification.md`.

## Rationale

The mental model requires one verification entry point that `claude`, `codex`, and `cursor-agent` all run, and defines the per-commit gate as the full gate relaxed for speed only. A POSIX shell script needs nothing beyond the pinned toolchain, runs identically in CI and in every harness's shell, and keeps the gate definition in one auditable file rather than spread over a task runner and three workflows. The only steps removed from the per-commit gate are the two that need network or a credential (dependency audit, CodeScene); everything else, including measured coverage, runs on every commit so that a PR into `staging` already knows it would pass `main`'s coverage threshold. The `justfile` exists because the owner's other repositories use `just`, not because the script needs it.

## Revisit when

A step becomes slow enough to move into the full gate only, or the CodeScene JSON field name in `scripts/codescene.sh` proves wrong once a token is available.
