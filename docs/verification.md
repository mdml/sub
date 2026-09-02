# Verification

`scripts/verify.sh` is the single verification entry point used by `claude`, `codex`, `cursor-agent`, maintainers, and CI. `just verify` and `just verify-full` are aliases. The current gate design is recorded in [`decisions/2026-09-02-verification-gates.md`](decisions/2026-09-02-verification-gates.md); the 2026-08-27 verification records remain historical.

## Setup

- Rust toolchain: `rustup` reads `rust-toolchain.toml` (1.97.1 with `rustfmt`, `clippy`, and `llvm-tools`).
- Tools: `mise install` provisions `just`, `cargo-llvm-cov`, and `cargo-deny` at the versions in `mise.toml`. Without `mise`, install those versions directly.
- CodeScene: every gate requires `CS_ACCESS_TOKEN`. `scripts/codescene.sh` uses `cs` from `PATH` or installs the official CLI under `target/codescene`; installation additionally requires `curl`. A missing token is always a hard failure, including when no changed Rust files are eligible.

## Fast per-commit gate: `scripts/verify.sh`

Run the fast gate before every commit after staging the intended changes. Locally it checks staged Rust files with `scripts/codescene.sh --staged`. On a push-triggered GitHub Actions run it checks Rust files changed by `HEAD` with `scripts/codescene.sh --commit HEAD`.

| Step | Command | Fails when |
|:--|:--|:--|
| Format | `cargo fmt --all -- --check` | Any file differs from `rustfmt` output. |
| Lint | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Any warning. Workspace lint policy is defined in the root `Cargo.toml`. |
| Typecheck and build | `cargo build --workspace --all-targets --all-features --locked` | Compilation fails or `Cargo.lock` is stale. |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` | Rustdoc emits a warning. |
| Tests and coverage | `cargo llvm-cov --workspace --all-features --locked --fail-under-lines 90 --summary-only` | A test fails or workspace line coverage is below 90%. |
| Changed-file code health | `scripts/codescene.sh --staged` locally; `scripts/codescene.sh --commit HEAD` on push CI | `CS_ACCESS_TOKEN` is missing, CodeScene fails, or any changed scorable Rust file has a score below 10. |

The test step includes the behavioral contract suite against `sub-harness-fake`. Real-harness mode remains opt-in and is used only by the on-machine harness-compatibility nightly; verification gates do not run real harnesses.

## Full PR gate: `scripts/verify.sh --full`

Every pull request into `staging` or `main` runs the full gate. It runs the format, lint, build, docs, tests, and coverage steps from the fast gate, then adds:

| Step | Command | Fails when |
|:--|:--|:--|
| Dependency policy | `cargo deny --locked check` | An advisory, disallowed license, wildcard dependency, banned dependency, or disallowed source is found. |
| PR-relative code health | `scripts/codescene.sh --base BASE` | `CS_ACCESS_TOKEN` is missing, CodeScene fails, or any scorable Rust file changed relative to the PR base has a score below 10. |

The base ref defaults to `origin/staging`. Override it with `scripts/verify.sh --full --base REF` or `SUB_VERIFY_BASE=REF scripts/verify.sh --full`. CI supplies `origin/${{ github.base_ref }}` and checks out full history.

`scripts/codescene.sh` also retains whole-tree mode as `scripts/codescene.sh` or `scripts/codescene.sh --all`. CodeScene documents JSON `score: null` as “no scorable code”; those files are reported as ineligible rather than passed as score 10.

## CI

| Workflow and required context | Trigger | Gate |
|:--|:--|:--|
| `per-commit.yml` / `verify` | Push to any branch except `main` and `staging` | Fast gate against the pushed commit. |
| `per-commit.yml` / `verify` | Pull request into `staging` | Full gate relative to `staging`. |
| `full.yml` / `verify-full` | Pull request into `main` | Full gate relative to `main`. |
| `nightly.yml` | Daily at 06:17 UTC; manual | Advisory check and dependency-freshness report. |
| `release.yml` | Pull requests for release planning; cargo-dist semantic-version tag pushes for publishing | Generated release plan/build or publication; see [`release.md`](release.md). |

The `verify` and `verify-full` job names are the required-check contexts used by the protected-branch rulesets. Dependabot proposes Cargo and GitHub Actions updates weekly into `staging`. On-machine nightlies are documented under [`nightlies/`](nightlies/README.md).

## Secrets

| Secret | Used by | Purpose |
|:--|:--|:--|
| `CS_ACCESS_TOKEN` | `per-commit.yml`, `full.yml` | Mandatory CodeScene authentication for push and PR gates. |
| `HOMEBREW_TAP_TOKEN` | `release.yml` | Push access to the Homebrew tap repository for releases. |
