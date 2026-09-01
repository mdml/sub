# Beta path: delegate → observe → recover → cancel

This scenario is the initial beta release gate. One real Codex task carries all four controls under one stable task handle: launch delegates the task, a fresh process observes it live, the attempt-1 supervisor is deliberately killed, explicit recovery resumes the exact harness session as attempt 2, and a fresh process cancels that resumed attempt. One task is used because it proves that recovery preserves the identity later consumed by Control; cancellation is terminal and therefore must be the final step.

The captured run was executed on 2026-09-01 with Codex CLI 0.151.0 and `@agentclientprotocol/codex-acp` 1.6.2.

## Prerequisites

Follow the [Control proof prerequisites](control.md#prerequisites). `$SUB_BIN` is the absolute path to the built `sub` binary. Use an empty throwaway `$WORK` and `$STATE_DIR`.

## Scenario

Delegate a task with a long live window and retain its handle:

```sh
"$SUB_BIN" launch --harness codex --cwd "$WORK" --prompt 'Create beta-before-recover.txt containing exactly beta before recover, then run sleep 300, then create beta-after-recover.txt containing exactly beta after recover, and report both files with Markdown links.' --binary "$(command -v codex)" --permission-mode agent --state-dir "$STATE_DIR"
```

From a fresh process, observe `running`, task-to-attempt linkage, a non-null harness session ID, and live activity:

```sh
"$SUB_BIN" inspect "$HANDLE" --state-dir "$STATE_DIR"
```

Resolve the attempt-1 supervisor PID from implementation-private state, verify that its command line is `$SUB_BIN __supervise "$HANDLE" 1`, and kill only that verified `sub` process. Observe `orphaned`, then recover:

```sh
kill -9 "$SUPERVISOR_PID"
"$SUB_BIN" inspect "$HANDLE" --state-dir "$STATE_DIR"
"$SUB_BIN" recover "$HANDLE" --state-dir "$STATE_DIR"
```

Wait until inspection reports attempt 2 `running`, the same harness session ID on both attempts, and `attempt_resumed`. From another fresh process, cancel and wait:

```sh
"$SUB_BIN" cancel "$HANDLE" --state-dir "$STATE_DIR"
"$SUB_BIN" wait "$HANDLE" --timeout-seconds 15 --state-dir "$STATE_DIR"
"$SUB_BIN" inspect "$HANDLE" --state-dir "$STATE_DIR"
```

## Expected proof

The final inspection reports attempt 1 `orphaned`, attempt 2 `cancelled`, one stable task handle and harness session ID, and the lifecycle sequence `attempt_started` → `attempt_orphaned` → `attempt_started` → `attempt_resumed` → `attempt_cancelled` → `attempt_finished`. Cancel returns `delivered`; wait returns the partial cancelled result with normalized and native evidence references intact. The resumed child states that it preserved elapsed work rather than replaying the original task, demonstrating recovery before cancellation.

Captured, scrubbed evidence is under [`../../proofs/beta-path/evidence/`](../../proofs/beta-path/evidence/).
