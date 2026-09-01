# Supervisor liveness and orphaned attempt status

Date: 2026-09-01. Status: adopted.

## Decision

Each running attempt durably records the supervisor process ID and an operating-system process start token. On Linux, observation reads `/proc/<pid>/stat` and requires both the PID and start token to match a non-zombie process; this distinguishes the recorded supervisor from a reused PID. On other supported Unix systems, observation falls back to `kill -0`, where no stronger process identity is available. Staleness timestamps are not used as liveness evidence.

`orphaned` is a distinct `TaskStatus`, neither running nor terminal failure. `Delegator::inspect`, `Delegator::list`, and `Delegator::wait` derive it when persisted state says `running` but the recorded supervisor is dead. Wait returns `{ "state": "orphaned", "status": "orphaned" }` immediately rather than timing out as running. Read-only observation does not mutate the append-only log; explicit recovery appends `attempt_orphaned` to the predecessor attempt before creating its successor.

## Rationale

A PID alone can be reused, and an age threshold can misclassify slow work. PID plus process start identity provides direct liveness evidence on Linux while keeping observation independent of a supervisor, bridge, or daemon. A distinct nonterminal status states that the harness attempt lost its owner but may still be continued through its recorded session.

## Revisit when

The supported platform set needs a start-identity mechanism beyond Linux `/proc`, a supervisor heartbeat becomes useful for information other than liveness, or state migration guarantees make the private process-identity fields public compatibility concerns.
