# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/). Public interfaces may break before 1.0.

## [Unreleased]

### Added

- Repository shell: README, Apache-2.0 license, agent instructions, this changelog.
- Spike report `docs/spikes/acp-boundary.md` with captured evidence under `spikes/acp-boundary/` (ACP v1 capability map for `claude`, `codex`, `cursor-agent`; recommendation: wrap ACP with a small delegation layer). The disposable prototype was removed at resolution and remains in git history.
- Cargo workspace with stub crates: `sub-sdk`, `sub-cli` (binary `sub`), `sub-mcp`, `sub-harness-fake`, `sub-adapter-claude`, `sub-adapter-codex`, `sub-adapter-cursor`. `tokio` 1.53.1 and `agent-client-protocol` 2.0.0 pinned exactly.
- Verification entry point `scripts/verify.sh` (per-commit gate) and `scripts/verify.sh --full` (full gate with `cargo-deny` and CodeScene), with `just` aliases; toolchain pinned by `rust-toolchain.toml` and tools by `mise.toml`.
- GitHub Actions: per-commit gate, full gate, nightly vulnerability and freshness checks, Dependabot, and a `cargo-dist` release workflow (no release cut).
- Project-level harness configuration for `claude`, `codex`, and `cursor-agent`.
- Documentation skeleton under `docs/` with decision records dated 2026-08-27, including the bridge pinning design.
- On-machine nightly job definitions under `docs/nightlies/` and `scripts/nightly/`.
