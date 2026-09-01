# `sub`

Give the coding agent you already use subagents from any supported coding harness, with one durable place to observe the delegated work and retrieve its result.

`sub` is in pre-beta. Claude Code and Codex can install their pinned ACP bridges, launch one bounded child task under an independent supervisor, wait on the returned durable handle, and independently observe task status, normalized events, reported cost, and reported tokens through CLI or MCP.

## Status

Current proofs: Delegate passes from a real Claude Code manager to a real Codex child; Observe passes live and after completion on real Claude Code and Codex tasks through independent CLI and MCP processes; Recover passes both replacement-manager wait and replacement-supervisor session resume. Control remains a later proof. See [`docs/proofs/delegate.md`](docs/proofs/delegate.md), [`docs/proofs/observe.md`](docs/proofs/observe.md), [`docs/proofs/recover.md`](docs/proofs/recover.md), and [`CHANGELOG.md`](CHANGELOG.md).

## Try launch, wait, and Observe

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

If wait returns `{"state":"running",...}`, call it again with the same handle. The MCP server binary is `target/debug/sub-mcp`; its `sub_bridge_install`, `sub_launch`, and `sub_wait` tools expose the same controls.

Observe from any process that can read the same state directory:

```sh
target/debug/sub list
target/debug/sub inspect tsk_REPLACE_WITH_HANDLE
```

The MCP server also exposes `sub_list` and `sub_inspect`. Both surfaces serialize the same SDK shapes. Unsupported usage is null beside `usage_support: false`; it is never reported as zero or estimated.

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
