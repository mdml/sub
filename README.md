# `sub`

Give the coding agent you already use subagents from any supported coding harness, with one durable place to observe the delegated work and retrieve its result.

`sub` is in pre-beta. Claude Code and Codex can install their pinned ACP bridges, launch one bounded child task under an independent supervisor, wait on the returned durable handle, independently observe task status and evidence, recover an orphaned harness session, and cancel a running child through CLI or MCP.

## Status

The four beta feature proofs pass: Delegate from a real Claude Code manager to a real Codex child, Observe through independent CLI and MCP processes, Recover through replacement-manager wait and replacement-supervisor session resume, and Control through cross-process cancellation. The composed delegate → observe → recover → cancel path also passes on one real task. See [`docs/proofs/delegate.md`](docs/proofs/delegate.md), [`docs/proofs/observe.md`](docs/proofs/observe.md), [`docs/proofs/recover.md`](docs/proofs/recover.md), [`docs/proofs/control.md`](docs/proofs/control.md), and [`docs/proofs/beta-path.md`](docs/proofs/beta-path.md).

## Try launch, wait, observe, recover, and cancel

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

If wait returns `{"state":"running",...}`, call it again with the same handle. If inspection reports `orphaned`, explicit recover creates the next attempt and resumes the recorded harness session. Cancel returns its delivery disposition immediately; observe or wait for the terminal result.

```sh
target/debug/sub recover tsk_REPLACE_WITH_HANDLE
target/debug/sub cancel tsk_REPLACE_WITH_HANDLE
```

The MCP server binary is `target/debug/sub-mcp`; `sub_bridge_install`, `sub_launch`, `sub_wait`, `sub_recover`, and `sub_cancel` expose the same controls.

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
