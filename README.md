# `sub`

Give the coding agent you already use subagents from any supported coding harness, with one durable place to observe the delegated work and retrieve its result.

`sub` is in pre-beta. There is nothing to install yet.

## Status

Current phase: the repository is scaffolded; the fake harness and contract suite come next. The Agent Client Protocol boundary spike is resolved (`docs/spikes/acp-boundary.md`). See [`CHANGELOG.md`](CHANGELOG.md).

## Build

Requirements: `rustup` (the toolchain in `rust-toolchain.toml` installs itself on first use) and, for the verification tools, [`mise`](https://mise.jdx.dev).

```sh
mise install          # just, cargo-llvm-cov, cargo-deny at pinned versions
cargo build --workspace
```

## Verify

```sh
scripts/verify.sh          # per-commit gate: format, lint, build, docs, tests with coverage
scripts/verify.sh --full   # full gate: adds dependency audit and CodeScene (needs CS_ACCESS_TOKEN)
```

`just verify` and `just verify-full` are aliases. See [`docs/verification.md`](docs/verification.md).

## Layout

A Cargo workspace under `crates/`: `sub-sdk` (kernel), `sub-cli` and `sub-mcp` (surfaces), `sub-harness-fake`, and one adapter crate per harness. See [`docs/architecture.md`](docs/architecture.md) and the index at [`docs/README.md`](docs/README.md).

## Developing with agents

Instructions for `claude`, `codex`, and `cursor-agent` are in [`AGENTS.md`](AGENTS.md); harness configuration is described in [`docs/harnesses.md`](docs/harnesses.md).

## License

Apache-2.0. See [`LICENSE`](LICENSE).
