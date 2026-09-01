# Delegate proof: Claude Code manager to Codex child

This scenario proves the first beta feature: a real Claude Code manager launches one bounded task on a real Codex child through `sub`, receives a task handle immediately, and retrieves the result with wait. It was executed on 2026-08-31 with Claude Code 2.1.251, Codex CLI 0.151.0, `@agentclientprotocol/claude-agent-acp` 0.70.0, and `@agentclientprotocol/codex-acp` 1.6.2.

## Prerequisites

- Build `sub`: `cargo build -p sub-cli --locked`.
- Install the pinned bridges through `sub`: `sub bridge install claude` and `sub bridge install codex`.
- Use the user's already-authenticated `claude` and `codex` binaries. `sub` does not handle credentials.
- Create an empty throwaway working directory and choose a throwaway state directory. The commands below use `$WORK`, `$STATE_DIR`, `$SUB_BIN`, and `$CODEX_BIN` placeholders.

## Manager prompt

Run a real Claude Code manager session with subagents disabled and give it this bounded instruction:

> Use Bash to run exactly one cross-harness delegation through `$SUB_BIN`. Launch Codex with cwd `$WORK`, binary `$CODEX_BIN`, permission mode `agent`, state directory `$STATE_DIR`, and prompt: `Create proof.txt containing exactly delegate proof passed, then report the changed file using a Markdown link.` Capture the returned handle. Run `sub wait` on that handle with timeout 120 seconds and the same state directory. If it is still running, repeat wait once. Return the launch JSON and final wait JSON. Do not perform the child task yourself and do not use subagents.

One non-interactive invocation is:

```sh
claude -p --output-format json --permission-mode bypassPermissions --disallowedTools Agent Task --max-budget-usd 2 "$MANAGER_PROMPT"
```

## Expected proof

Launch returns `{"id":"tsk_..."}` before the child finishes. Wait returns `state: complete`, `status: succeeded`, `changed_files` containing `$WORK/proof.txt`, artifact references including the native Codex session record, and a Codex harness session ID. The file contains exactly `delegate proof passed`. The manager's output reports zero spawned subagents.

Captured, scrubbed outputs are under [`../../proofs/delegate/evidence/`](../../proofs/delegate/evidence/). The first execution exposed a Codex bridge behavior: file creation through an execute tool had no ACP edit location. The shipped result derivation also folds existing in-workdir Markdown file links from the final stream; the final captured proof shows `changed_files` populated.

This feature composes into the release-gating [beta path](beta-path.md).
