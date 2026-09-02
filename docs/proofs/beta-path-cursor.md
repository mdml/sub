# Cursor variant: delegate → observe → recover → cancel

This non-gating variant reruns the composed beta path with Cursor Agent as the child. A Codex-side manager launched one real Cursor task under a stable `sub` handle, observed it live, verified and killed only its attempt-1 `sub __supervise` process, recovered the same Cursor session through ACP `session/load`, and cancelled attempt 2 through the public CLI controls. The mental model keeps the Claude-to-Codex beta path as the release gate, so this first-release Cursor variant is additional compatibility evidence.

The captured run was executed on 2026-09-01 with Cursor Agent 2026.08.25-3e8eec8. Every `SUB_CONFIG`, state, work, skill, and MCP destination used a throwaway root. No real Cursor configuration was modified.

## Scenario

Configure `[harnesses.cursor]` with the installed `cursor-agent` binary and native `agent` permission mode. Launch a deliberately long no-tool response so the supervisor can be interrupted without a permission prompt ending the turn:

```sh
sub launch --harness cursor --cwd "$WORK" --prompt 'Output the integers from 1 through 100000, one integer per line. Do not abbreviate, skip, use tools, or finish early.'
```

From another process, inspect until attempt 1 is `running` with a non-null harness session ID. Read the implementation-private supervisor PID, verify its command line is the expected `sub __supervise "$HANDLE" 1 --state-dir "$STATE_DIR"`, and kill only that process. Inspect `orphaned`, then use public recovery:

```sh
sub recover "$HANDLE"
sub inspect "$HANDLE"
```

Inspection reports attempt 2 `running`, the same harness session ID on both attempts, and `attempt_resumed`. Cancel from another process and retrieve the terminal result:

```sh
sub cancel "$HANDLE"
sub wait "$HANDLE" --timeout-seconds 15
sub inspect "$HANDLE"
```

## Captured result

The final task is `cancelled`; attempt 1 is `orphaned`, attempt 2 is `cancelled`, and both use `$CURSOR_SESSION_ID`. The continuation summary begins with “Continuing from 1001 (after 1–1000 in the prior attempt),” direct behavioral evidence that load replay restored the interrupted conversation rather than silently creating a fresh session. `attempt_resumed` precedes live attempt-2 activity. `usage_support` is false for cost and tokens, and task plus per-attempt usage remain null. Cancel returned `delivered`; the terminal events report `attempt_cancelled { harness_honored: true }` and `attempt_finished { status: cancelled }`.

Scrubbed captured evidence is under [`../../proofs/beta-path-cursor/evidence/`](../../proofs/beta-path-cursor/evidence/). Replay updates emitted before the `session/load` response are intentionally absent from attempt-2 activity under the [load-replay observation decision](../decisions/2026-09-01-load-replay-observation-boundary.md).
