# `sub`

Give the coding agent you already use subagents from any supported coding harness, with one durable place to observe the delegated work and retrieve its result.

`sub` is in pre-beta. Claude Code and Codex can install their pinned ACP bridges, launch one bounded child task under an independent supervisor, and wait on the returned durable handle through CLI or MCP.

## Status

Current proof: Delegate passes from a real Claude Code manager to a real Codex child. Observe, Recover, and Control remain later proofs. See [`docs/proofs/delegate.md`](docs/proofs/delegate.md) and [`CHANGELOG.md`](CHANGELOG.md).

## Try launch and wait

Install each exact pinned bridge once. `npm` is required only for these explicit install commands.

```sh
cargo build -p sub-cli
target/debug/sub bridge install claude
target/debug/sub bridge install codex
```

Launch requires the child harness, bounded prompt, existing working directory, the user's harness binary, and a harness-native permission mode. It returns JSON immediately.

```sh
target/debug/sub launch --harness codex --cwd "$PWD" --prompt "Review the current change and report findings." --binary "$(command -v codex)" --permission-mode read-only
target/debug/sub wait tsk_REPLACE_WITH_HANDLE --timeout-seconds 30
```

If wait returns `{"state":"running",...}`, call it again with the same handle. The MCP server binary is `target/debug/sub-mcp`; it exposes `sub_bridge_install`, `sub_launch`, and `sub_wait` with the same fields.

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
