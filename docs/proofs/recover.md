# Recover proof: replacement manager and replacement supervisor

This scenario proves the third beta feature through its two distinct mechanisms. A manager process can die while its detached supervisor continues, and a fresh process can wait on the same task handle without reconstructing the transcript. A supervisor can die mid-attempt, producing an orphaned attempt; explicit recovery creates attempt 2, resumes the same harness session through a fresh supervisor, and completes the task. It was executed on 2026-09-01 with Codex CLI 0.151.0 and `@agentclientprotocol/codex-acp` 1.6.2. The real-harness contract mode also passed with Claude Code 2.1.252 through `@agentclientprotocol/claude-agent-acp` 0.70.0.

## Prerequisites

- Build `sub` and `sub-mcp`: `cargo build -p sub-cli -p sub-mcp --locked`.
- Install the pinned bridges into a throwaway state directory: `sub bridge install claude --state-dir "$STATE_DIR"` and `sub bridge install codex --state-dir "$STATE_DIR"`.
- Use the user's already-authenticated `claude` and `codex` binaries. `sub` does not handle credentials.
- Create empty throwaway `$MANAGER_WORK`, `$SUPERVISOR_WORK`, and `$STATE_DIR` directories. `$SUB_BIN`, `$SUB_MCP_BIN`, and `$CODEX_BIN` below are absolute binary paths.

## Leg 1: manager death

Start a disposable manager shell that launches the task, records the handle, and remains alive while the supervisor works:

```sh
sh -c '"$SUB_BIN" launch --harness codex --cwd "$MANAGER_WORK" --prompt "Run sleep 12, then create manager-death.txt containing exactly manager recovery passed, and report the file with a Markdown link." --binary "$CODEX_BIN" --permission-mode agent --state-dir "$STATE_DIR" >"$MANAGER_HANDLE_FILE"; sleep 120' &
MANAGER_PID=$!
```

After `$MANAGER_HANDLE_FILE` contains the launch JSON and `sub inspect` reports the attempt running, kill only `$MANAGER_PID`. From a fresh shell process, extract the handle and wait:

```sh
kill "$MANAGER_PID"
HANDLE=$(jq -r .id "$MANAGER_HANDLE_FILE")
"$SUB_BIN" wait "$HANDLE" --timeout-seconds 60 --state-dir "$STATE_DIR"
```

The captured result is `succeeded`, reports `manager-death.txt`, and retains the native Codex session by reference. No recovery attempt is created because the detached attempt-1 supervisor never died.

## Leg 2: supervisor death

Launch a task with a deliberate live window, wait until `sub inspect` reports `running` with a non-null `harness_session_id`, and resolve the supervisor PID from the implementation-private attempt state:

```sh
"$SUB_BIN" launch --harness codex --cwd "$SUPERVISOR_WORK" --prompt 'Run sleep 30, then create supervisor-death.txt containing exactly supervisor recovery passed, and report the file with a Markdown link.' --binary "$CODEX_BIN" --permission-mode agent --state-dir "$STATE_DIR"
"$SUB_BIN" inspect "$HANDLE" --state-dir "$STATE_DIR"
SUPERVISOR_PID=$(jq -r .supervisor_pid "$STATE_DIR/tasks/$HANDLE/attempts/1/state.json")
kill -9 "$SUPERVISOR_PID"
"$SUB_BIN" inspect "$HANDLE" --state-dir "$STATE_DIR"
```

The second inspect reports both task and attempt 1 as `orphaned`, not running or failed. Recover and wait from fresh processes:

```sh
"$SUB_BIN" recover "$HANDLE" --state-dir "$STATE_DIR"
"$SUB_BIN" wait "$HANDLE" --timeout-seconds 90 --state-dir "$STATE_DIR"
"$SUB_BIN" inspect "$HANDLE" --state-dir "$STATE_DIR"
```

Recovery returns the same task handle and `attempt: 2`. The final inspection shows attempt 1 `orphaned`, attempt 2 `succeeded`, the same harness session ID on both attempts, `attempt_orphaned` and `attempt_resumed`, per-attempt usage, and the accumulated task usage. The child explicitly reported continuing the interrupted wait without starting another delay, which demonstrates continuation rather than original-prompt replay.

## MCP parity

Call `sub_inspect` from a fresh `sub-mcp` process with the same handle and state directory. Its `structuredContent` is the same `TaskInspection` shape as CLI output. `sub_recover` accepts `{ "handle": HANDLE, "state_dir": STATE_DIR }` and returns the same `RecoverOutcome` shape as CLI: `{ "handle": { "id": HANDLE }, "attempt": 2 }`.

## Contract results and evidence

The fake behavioral contract covers successful cross-process resume, bridge refusal, and a missing persisted session ID. The fake `session/load` implementation replays the fixture stream on load, matching the ACP boundary evidence for replay-loading bridges. Real-harness contract mode created a session in one bridge process and resumed the same ID in a fresh process for Claude and Codex; both passed, and neither disagreed with the fake behavior exercised for its resume mechanism.

Captured, scrubbed evidence is under [`../../proofs/recover/evidence/`](../../proofs/recover/evidence/). Paths use `$STATE_DIR`, `$WORK`, and `$CODEX_HOME`; the killed supervisor PID and machine identity are omitted. `manager-death.json` captures the fresh-process wait, `orphaned-cli.json` captures direct liveness detection, `recover-cli.json` captures attempt creation, `complete-cli.json` and `complete-mcp.json` capture session lineage and usage, and `contract-results.json` records fake and real contract outcomes.
