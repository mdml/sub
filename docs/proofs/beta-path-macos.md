# macOS: delegate → observe → recover → cancel

This run is the real-harness platform proof for macOS on Apple silicon (`aarch64-apple-darwin`). It repeats the [beta path](beta-path.md) scenario unchanged on that platform: a real Claude Code manager delegates one real Codex task through the `sub` CLI, a fresh process observes it live, the verified attempt-1 supervisor is killed, explicit recovery resumes the same Codex session as attempt 2, and a fresh process cancels the resumed attempt. Under the mental model's distribution rule, this proof is what returns `aarch64-apple-darwin` to the shipped release targets. It does not demonstrate Intel macOS (`x86_64-apple-darwin`).

The captured run was executed on 2026-10-02 on macOS 27.0.1 (arm64) with a debug build of `staging` at `aa4fe0a`, Codex CLI 0.159.2, `@agentclientprotocol/codex-acp` 1.6.2, and Claude Code 2.1.285 as the manager. Every state and work directory was a throwaway root.

## Prerequisites and scenario

Follow the [beta path prerequisites and scenario](beta-path.md#prerequisites) as written. No macOS-specific step is needed: the supervisor PID and start identity come from the same implementation-private attempt state, and the command line is verified with `ps -o command= -p "$SUPERVISOR_PID"` before `kill -9`.

## Captured result

The final inspection reports attempt 1 `orphaned`, attempt 2 `cancelled`, one stable task handle, and one Codex session ID on both attempts. The lifecycle sequence is `attempt_started` → `attempt_orphaned` → `orphaned_child_disposed` → `attempt_started` → `attempt_resumed` → `attempt_cancelled { harness_honored: true }` → `attempt_finished { status: cancelled }`. `orphaned_child_disposed` reports `already_gone`: the recorded bridge child had exited after its supervisor was killed, so recovery left it alone and attempt 2 was the task's only live child. Cancel returned `delivered`; wait returned the partial cancelled result with the event log, supervisor log, and native Codex session references intact. Token usage is reported (`usage_support.tokens` is true) and cost is not.

The resumed child's summary — “The first file is created. I’ll resume waiting for the existing `sleep 300`, then create the second file.” — is behavioral evidence that the resumed session continued the interrupted work rather than replaying the task. After cancellation, the work directory held only `beta-before-recover.txt`, and no process for the handle remained.

Scrubbed captured evidence is under [`../../proofs/beta-path-macos/evidence/`](../../proofs/beta-path-macos/evidence/).
