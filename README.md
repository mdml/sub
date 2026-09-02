# `sub`

Give the coding agent you already use subagents from any supported coding harness, with one durable place to observe the delegated work and retrieve its result.

`sub` is in pre-beta. Claude Code, Codex, and Cursor Agent can be configured and onboarded, launch one bounded child task under an independent supervisor, wait on the returned durable handle, independently observe task status and evidence, recover an orphaned harness session, and cancel a running child through CLI or MCP.

## Status

The four beta feature proofs pass: Delegate from a real Claude Code manager to a real Codex child, Observe through independent CLI and MCP processes, Recover through replacement-manager wait and replacement-supervisor session resume, and Control through cross-process cancellation. The composed delegate → observe → recover → cancel path passes on one real Codex task; a non-gating Cursor-child variant passes through replaying `session/load`. See [`docs/proofs/delegate.md`](docs/proofs/delegate.md), [`docs/proofs/observe.md`](docs/proofs/observe.md), [`docs/proofs/recover.md`](docs/proofs/recover.md), [`docs/proofs/control.md`](docs/proofs/control.md), [`docs/proofs/beta-path.md`](docs/proofs/beta-path.md), and [`docs/proofs/beta-path-cursor.md`](docs/proofs/beta-path-cursor.md).

## Configure and onboard

Create `$XDG_CONFIG_HOME/sub/sub.toml`, or `$HOME/.config/sub/sub.toml` when `XDG_CONFIG_HOME` is unset. `SUB_CONFIG` selects another path for tests. The beta schema contains only a state-directory override and harness binary, model, and permission-mode defaults:

```toml
state_dir = "/home/alice/.local/state/sub"

[harnesses.claude]
binary = "/home/alice/.local/bin/claude"
permission_mode = "bypassPermissions"

[harnesses.codex]
binary = "/home/alice/.local/bin/codex"
model = "gpt-5"
permission_mode = "agent"

[harnesses.cursor]
binary = "/home/alice/.local/bin/cursor-agent"
permission_mode = "agent"
```

Build both surfaces, then explicitly onboard only the requested manager harnesses. `npm` is required while onboarding installs the exact pinned Claude and Codex bridges; Cursor Agent speaks ACP v1 natively, so its bridge step reports `not_required`. The same action installs the `sub-delegation` manager skill and registers the adjacent `sub-mcp` binary in each named harness's user configuration.

```sh
cargo build -p sub-cli -p sub-mcp
target/debug/sub onboard claude codex cursor
```

Re-running onboarding repairs stale files or reports `unchanged`; it never configures an unnamed harness. `sub` uses each harness's existing authentication and never holds credentials. See [`docs/decisions/2026-09-01-onboarding-installation.md`](docs/decisions/2026-09-01-onboarding-installation.md).

## Try launch, wait, observe, recover, and cancel

With a configured harness, launch needs only the child harness, bounded prompt, and existing working directory. It returns JSON immediately.

```sh
target/debug/sub launch --harness codex --cwd "$PWD" --prompt "Review the current change and report findings."
target/debug/sub wait tsk_REPLACE_WITH_HANDLE --timeout-seconds 30
```

If wait returns `{"state":"running",...}`, call it again with the same handle. If inspection reports `orphaned`, explicit recover creates the next attempt and resumes the recorded harness session. Cancel returns its delivery disposition immediately; observe or wait for the terminal result.

```sh
target/debug/sub recover tsk_REPLACE_WITH_HANDLE
target/debug/sub cancel tsk_REPLACE_WITH_HANDLE
```

Explicit `--binary`, `--model`, `--permission-mode`, and `--state-dir` values override `sub.toml`; the prior fully explicit launch form remains valid. No executable is guessed from `PATH`. The MCP server binary is `target/debug/sub-mcp`; `sub_bridge_install`, `sub_launch`, `sub_wait`, `sub_recover`, and `sub_cancel` expose the same controls and optional launch overrides.

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
