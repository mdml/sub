# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/). Public interfaces may break before 1.0.

## [Unreleased]

### Added

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
