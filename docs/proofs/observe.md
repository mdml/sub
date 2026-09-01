# Observe proof: independent live and completed inspection

This scenario proves the second beta feature: processes other than the launching manager inspect real Claude Code and Codex tasks through CLI and MCP while the supervisors run and after they finish. It was executed on 2026-09-01 with Claude Code 2.1.252, Codex CLI 0.151.0, `@agentclientprotocol/claude-agent-acp` 0.70.0, and `@agentclientprotocol/codex-acp` 1.6.2.

## Prerequisites

- Build `sub` and `sub-mcp`: `cargo build -p sub-cli -p sub-mcp --locked`.
- Install the pinned bridges through `sub`: `sub bridge install claude --state-dir "$STATE_DIR"` and `sub bridge install codex --state-dir "$STATE_DIR"`.
- Use already-authenticated `claude` and `codex` binaries. `sub` does not handle credentials.
- Create empty throwaway `$CLAUDE_WORK`, `$CODEX_WORK`, and `$STATE_DIR` directories. Run each observation command from a shell process other than the launching manager.

## Scenario

Launch both real tasks. The intentional delay leaves a live-observation window.

```sh
sub launch --harness claude --cwd "$CLAUDE_WORK" --prompt 'Run `sleep 8`, then create observe-claude.txt containing exactly `observe claude passed`, and report the file.' --binary "$(command -v claude)" --permission-mode bypassPermissions --state-dir "$STATE_DIR"
sub launch --harness codex --cwd "$CODEX_WORK" --prompt 'Run `sleep 8`, then create observe-codex.txt containing exactly `observe codex passed`, and report the file.' --binary "$(command -v codex)" --permission-mode agent --state-dir "$STATE_DIR"
```

While each task is running, use fresh CLI and MCP processes:

```sh
sub list --state-dir "$STATE_DIR"
sub inspect "$HANDLE" --state-dir "$STATE_DIR"
printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"sub_inspect\",\"arguments\":{\"handle\":\"$HANDLE\",\"state_dir\":\"$STATE_DIR\"}}}" | sub-mcp
```

After `sub wait "$HANDLE" --timeout-seconds 60 --state-dir "$STATE_DIR"` completes, repeat both inspect calls. Verify `observe-claude.txt` and `observe-codex.txt` contain their requested exact lines.

## Expected proof

Live CLI and MCP output report `status: running`, task-to-attempt linkage, `attempt_started`, and streamed activity without contacting or modifying the supervisor. Completed output reports `status: succeeded`, `attempt_finished`, and usage accumulated for the attempt and task. Claude reports USD cost and per-turn tokens. Codex reports per-turn tokens, while `usage_support.cost: false` and `usage.cost: null` state that its harness did not report cost. No event contains transcript text; the native transcript remains an artifact reference in the task result.

Captured, scrubbed evidence is under [`../../proofs/observe/evidence/`](../../proofs/observe/evidence/). `live-cli.json` and `live-mcp.json` show independent running observations. `complete-cli-claude.json` and `complete-mcp-codex.json` show the two supported-usage combinations after completion. `contract-results.json` records the real-harness contract results and fake divergence check.
