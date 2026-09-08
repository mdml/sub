# Terminal cancel on an orphaned attempt

Date: 2026-09-08. Status: adopted. Supersedes in part [Cross-process cancel request signalling](2026-09-01-cancel-request-signalling.md).

## Decision

`Delegator::cancel` on a task whose latest attempt is orphaned no longer returns without writing. The kernel ends the attempt itself, in this order: append `attempt_orphaned`; dispose of the recorded harness child; append `orphaned_child_disposed { disposition }`; write the cancelled result; write `cancelled` attempt state; append `attempt_cancelled { harness_honored: false }` and `attempt_finished { status: cancelled }`. The delivery disposition stays `attempt_orphaned`, now meaning that the kernel cancelled the orphaned attempt directly and the result is already durable. Wait returns the cancelled result, and recover appends `attempt_recovery_rejected { reason: cancelled }` and fails, exactly as after any other cancel. CLI `sub cancel` and MCP `sub_cancel` gain the case without new commands.

Every supervisor records its bridge child's PID, operating-system start token, and process group in attempt state as soon as the ACP client spawns the child, through a process observer on `PromptOptions`. `HarnessChild::observe` exposes the same identity derivation so tests and proofs do not reimplement platform process queries. Orphaned cancel signals the child only when a live process still carries both the recorded PID and start token. It sends `SIGTERM` to the child's process group when the child leads that group, otherwise to the PID alone, waits the live-cancel grace period of five seconds, then sends `SIGKILL` and waits the same bound. Dispositions are `terminated`, `survived_termination`, `already_gone`, `identity_unverified` (a live process has the PID but another start token), and `identity_unrecorded`. An unverified or unrecorded PID is never signalled.

The cancelled result carries the recorded harness session identity and native-session reference when known, an empty changed-file list, and a summary stating that the supervisor died before deriving a result. No stream evidence is reconstructed.

## Rationale

The mental model states that cancel is terminal for the delegated task and that the child does not outlive the supervisor, and that recovery is never automatic. Requiring recover before cancel made the only route to ending orphaned work a new attempt that resumes the harness session, which starts work in order to stop it. Ending the attempt from the kernel keeps cancel terminal in every state. Signalling only a start-token-verified process keeps PID reuse from killing an unrelated process, the same guard observation already applies to supervisors.

## Revisit when

ACP defines a portable way to reach a harness session without its original client process, supported harnesses require a cleanup signal other than `SIGTERM`, or a daemon takes ownership of child processes across supervisor restarts.
