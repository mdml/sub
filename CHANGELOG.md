# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/). Public interfaces may break before 1.0.

## [Unreleased]

### Changed

- Rename the public harness identifier `cursor` to `cursor-agent` across `sub.toml` (`[harnesses.cursor-agent]`), CLI `--harness` and `sub onboard` values, MCP `harness` enum values, serialized task state, onboarding reports, native-session locators, and the `harness:cursor-agent` issue label; public harness identifiers are now the harness binary names. Existing task state written with `cursor` or `cursor_agent` still loads.
- Make cancel terminal for an orphaned attempt: `sub cancel` / `sub_cancel` on a task whose supervisor died now end the recorded harness child when its PID and start identity verify, publish a cancelled result, record `attempt_orphaned`, `orphaned_child_disposed`, `attempt_cancelled`, and `attempt_finished`, and reject later recovery; supervisors record the bridge child's identity in attempt state at spawn.
- Ship nightly and stable releases only for Linux x86_64 and aarch64 until a real-harness beta-path proof is recorded on macOS; retain macOS supervision and fake-harness CI as preparation.
- Stamp nightly build checkouts with their prerelease tag version, verify `sub --version` before packaging, and document mise's replacement GitHub backend for Release installs.
- Make detached supervision and PID-reuse-safe orphan detection uniform across Linux and macOS by using in-process `setsid(2)` and platform process start identities; force-terminate and reap an ignored-cancel child before publishing its cancelled result.
- Establish `staging`, `main`, and `stable` as development, nightly, and assembled stable channels, with merge-commit promotion into `main`, workflow-owned tags, whole-tree release gates, stable-only Homebrew publishing, and automated release-prep reconciliation to `staging`.
- Require the full verification gate on every pull request and the fast gate on every branch commit, with CodeScene score 10 enforced for the appropriate changed-file set and mandatory credentials in both CI workflows.
- Refactor the delegation kernel, ACP client, CLI, MCP, fake harness, and contract tests into focused modules while preserving public and serialized shapes; the whole tracked Rust tree now establishes a CodeScene health-10 baseline for all scorable files.
- Align harness-nightly and release-trigger documentation with the implemented scripts and workflows.

### Fixed

- Remove Linux aarch64 from nightly and stable release targets because it also lacks a recorded real-harness beta-path proof; Linux x86_64 is the only shipped target, and Linux aarch64 and macOS return after a proof is recorded for each platform.

- Restore the unconditional GitHub nightly vulnerability check so advisories are checked even when `main` has not moved, while retaining full verification only for changed release candidates.

### Added

- Add a clearly named macOS CI job that runs the workspace tests and fake-harness contract without real harnesses or CodeScene.
- Nightly GitHub prereleases built outside cargo-dist tag planning, a preview/confirm stable-promotion workflow with `stable-candidate` defaults and 0.x Conventional Commit versioning, and deploy-key-only branch/tag rulesets.
- Structured bug and feature issue forms, harness/regression/stable-candidate labels, reporting-flow documentation, and CLI `sub report <handle>` for scrubbed, reviewable issue commands without submission or credential access.
- A `sub`-failure section in the installed delegation skill that hands report drafts to the user and forbids managers from filing issues.
- Real `sub-adapter-cursor` support under the `cursor-agent` harness name: native `cursor-agent acp` launch with no bridge, native permission modes, prompt-level subagent guard plus Cursor task observation, false/false usage support, `session/load` recovery without replay double counting, onboarding, a stamped real-stream fixture and contract run, and a non-gating Cursor beta-path variant.
- Beta-minimum `sub.toml` discovery with configured state directory and per-harness binary, model, and permission-mode defaults shared by CLI and MCP launch; explicit arguments retain precedence.
- Idempotent `sub onboard <claude|codex>...` for pinned bridge installation, the `sub-delegation` manager skill, and user-level MCP registration, with per-harness action reports and throwaway-root overrides.
- Unit and integration coverage for config discovery, launch precedence, scoped onboarding, repair/idempotence, and matching CLI/MCP `LaunchParams`.
- Re-runnable, non-gating configure-and-onboard scenario with scrubbed evidence, a real config-only Codex launch, and real Claude/Codex contract results.
- Explicit Control operations: SDK `Delegator::cancel`, CLI `sub cancel`, and MCP `sub_cancel`, with immediate `delivered`, `already_finished`, or `attempt_orphaned` dispositions for one task handle.
- Supervisor-mediated ACP cancellation through durable per-attempt request markers, a five-second grace period, honest honored/ignored cancellation events, terminal partial results, and explicit rejection of recovery after cancellation.
- Fake and real Claude/Codex cancellation contracts, the re-runnable Control proof, and the composed delegate → observe → recover → cancel beta-path proof with scrubbed evidence.
- Explicit Recover controls: SDK `Delegator::recover`, CLI `sub recover`, and MCP `sub_recover`, which create attempt N+1 and resume an orphaned Claude or Codex harness session through a fresh detached supervisor.
- Direct supervisor liveness evidence with PID-reuse protection on Linux, the distinct `orphaned` task status, resume lifecycle/failure events, sequential attempt observation, and task-level usage accumulation across attempts.
- Fake-harness resume acceptance, refusal, missing-session coverage, real Claude/Codex cross-process resume contracts, and the two-leg Recover proof with scrubbed evidence.
- Read-only Observe controls: CLI `sub list` and `sub inspect`, MCP `sub_list` and `sub_inspect`, and shared SDK list/inspection types that work independently from a running or completed supervisor.
- Typed normalized task events for task/attempt linkage, attempt lifecycle, coalesced activity, and accumulated usage without transcript duplication.
- Per-turn token capture through the ACP SDK's unstable usage feature and streamed cost capture, with explicit harness support and absent—not zero—unreported measurements.
- Fake-harness fixtures with and without usage, live and terminal Observe contract coverage, real Claude/Codex usage assertions, and a re-runnable Observe proof with scrubbed evidence.
- Delegated-work kernel in `sub-sdk`: opaque task handles, one persisted execution attempt, independent per-attempt supervisors, append-only events, repeatable bounded wait, and results derived from ACP streams and stop reasons.
- Claude Code and Codex adapters with exact bridge/version declarations, user-harness binary selection, permission-mode/model forwarding, subagent restrictions, and native session records retained by reference.
- Explicit pinned bridge installation through CLI and MCP with per-version manifests and SHA-256 tree integrity checks; launch never fetches a bridge.
- CLI commands `sub bridge install`, `sub launch`, and `sub wait`; MCP tools `sub_bridge_install`, `sub_launch`, and `sub_wait` with matching controls.
- Re-runnable Claude-manager to Codex-child Delegate proof and scrubbed captured evidence under `docs/proofs/` and `proofs/delegate/`.

- Shared ACP client layer in `sub-sdk` (`sub_sdk::acp`): spawn agent over stdio, protocol v1, session + prompt, update stream, deny and surface permission requests, cancel, timeout.
- Programmable fake harness binary `sub-harness-fake` with fixture replay and scenario scripting (`replay`, `hang`, `die_mid_stream`, `ignore_cancel`, `cancel_honored`, `malformed`).
- Initial fixtures from spike evidence (`codex-hello`) and synthetic minimal streams; decision records for fixture and scenario formats (2026-08-31).
- Behavioral contract suite in `sub-sdk` (fake harness in CI; opt-in real-harness mode via `SUB_CONTRACT_REAL_HARNESS`).
- Harness-compatibility nightly script invokes real-harness contract mode; adapter version comparison remains a stub until adapters land.
- Repository shell: README, Apache-2.0 license, agent instructions, this changelog.
- Spike report `docs/spikes/acp-boundary.md` with captured evidence under `spikes/acp-boundary/` (ACP v1 capability map for `claude`, `codex`, `cursor-agent`; recommendation: wrap ACP with a small delegation layer). The disposable prototype was removed at resolution and remains in git history.
- Cargo workspace with crates: `sub-sdk`, `sub-cli` (binary `sub`), `sub-mcp`, `sub-harness-fake`, `sub-adapter-claude`, `sub-adapter-codex`, `sub-adapter-cursor`. `tokio` 1.53.1 and `agent-client-protocol` 2.0.0 pinned exactly.
- Verification entry point `scripts/verify.sh` (per-commit gate) and `scripts/verify.sh --full` (full gate with `cargo-deny` and CodeScene), with `just` aliases; toolchain pinned by `rust-toolchain.toml` and tools by `mise.toml`.
- GitHub Actions: per-commit gate, full gate, nightly vulnerability and freshness checks, Dependabot, and a `cargo-dist` release workflow (no release cut).
- Project-level harness configuration for `claude`, `codex`, and `cursor-agent`.
- Documentation skeleton under `docs/` with decision records dated 2026-08-27, including the bridge pinning design.
- Project-specific harness-compatibility nightly definition under `docs/nightlies/` and `scripts/nightly/`.
