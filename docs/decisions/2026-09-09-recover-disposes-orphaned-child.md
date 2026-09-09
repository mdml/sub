# Recover disposes of the orphaned attempt's child

Date: 2026-09-09. Status: adopted. Supersedes in part [Recovery attempts and bridge session resume](2026-09-01-recovery-resume-mechanics.md).

## Decision

`Delegator::recover` deals with the orphaned attempt's recorded harness child before it spawns the supervisor for attempt N+1. After the orphaned check and the `attempt_orphaned` event on attempt N, it runs the same verified-identity disposal that terminal cancel on an orphaned attempt uses, and appends `orphaned_child_disposed { disposition }` to attempt N. Only then does it write attempt N+1's request and state and spawn its supervisor.

The disposal path is unchanged and shared, not duplicated: a child is signalled only when a live process still carries both the recorded PID and start token, with `SIGTERM` to its process group where the child leads one, the live-cancel grace period, then `SIGKILL`. The dispositions are the existing `terminated`, `survived_termination`, `already_gone`, `identity_unverified`, and `identity_unrecorded`. An unverified or unrecorded identity is never signalled.

Every disposition is recorded and none of them stops the resume. Recover returns `RecoverOutcome { handle, attempt }` as before, and CLI `sub recover` and MCP `sub_recover` gain the behavior with no new commands, arguments, or response fields.

## Rationale

The mental model states that a recovered task has one live child. Without this step a recovered task could carry two: attempt 1's bridge and harness, still live because only its supervisor died, and attempt 2's, resuming the same harness session in the same directory. Two children writing the same working tree is exactly the interference the delegated-work model exists to keep out of the manager's way, and the first child is unreachable — no supervisor is left to deliver ACP cancellation to it.

Disposal is best-effort by design. An orphan that is already gone or whose identity cannot be verified is left alone rather than risking a signal to an unrelated process that inherited the PID, and recovery proceeds either way because refusing to resume would leave the manager with no route forward.

## Revisit when

A daemon takes ownership of child processes across supervisor restarts, or a supported harness requires its child to survive the supervisor that spawned it.
