# Control proof: cross-process cancellation of a real task

This scenario proves the fourth beta feature: a process other than the launching manager requests cancellation of a running real Codex child, then Observe and wait expose a terminal cancelled attempt and its partial evidence. It was executed on 2026-09-01 with Codex CLI 0.151.0 and `@agentclientprotocol/codex-acp` 1.6.2. The same contract mode passed with Claude Code 2.1.252 and `@agentclientprotocol/claude-agent-acp` 0.70.0.

## Prerequisites

- Build `sub` and `sub-mcp`: `cargo build -p sub-cli -p sub-mcp --locked`.
- Install the pinned bridges into a throwaway state directory: `sub bridge install claude --state-dir "$STATE_DIR"` and `sub bridge install codex --state-dir "$STATE_DIR"`.
- Use already-authenticated `claude` and `codex` binaries. `sub` does not handle credentials.
- Create empty throwaway `$WORK` and `$STATE_DIR` directories. Run cancellation and observation from processes other than the launching process.

## Scenario

Launch work that creates evidence before entering a long-running step:

```sh
sub launch --harness codex --cwd "$WORK" --prompt 'Create partial-control.txt containing exactly control partial evidence, then run sleep 30, then report the file with a Markdown link.' --binary "$(command -v codex)" --permission-mode agent --state-dir "$STATE_DIR"
```

After `sub inspect "$HANDLE" --state-dir "$STATE_DIR"` reports `running`, issue cancellation from a fresh process and return immediately:

```sh
sub cancel "$HANDLE" --state-dir "$STATE_DIR"
sub wait "$HANDLE" --timeout-seconds 15 --state-dir "$STATE_DIR"
sub inspect "$HANDLE" --state-dir "$STATE_DIR"
```

For MCP parity, call `sub_cancel` with `{ "handle": HANDLE, "state_dir": STATE_DIR }`. It returns the same `CancelOutcome` shape as the CLI.

## Expected proof

Cancel returns `{handle, attempt: 1, delivery: "delivered"}` without waiting for the grace period. Wait returns `state: complete` with result status `cancelled`. Inspect reports task and attempt status `cancelled`, `attempt_cancelled` with `harness_honored: true`, `attempt_finished`, and partial token usage. The result retains the assistant text streamed before cancellation, the harness session ID, the normalized event log, supervisor log, and native session reference. `partial-control.txt` remains intact.

The captured Codex stream did not attach an ACP edit location to the execute tool that created `partial-control.txt`, and cancellation prevented a final Markdown link. The partial result therefore reports an empty `changed_files` list while the file and native evidence remain intact; this is the stream-derived result rather than an inferred filesystem diff.

Captured, scrubbed evidence is under [`../../proofs/control/evidence/`](../../proofs/control/evidence/). The complete beta composition is the [beta path](beta-path.md).
