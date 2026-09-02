# Owned process-group termination after cancellation grace

Date: 2026-09-02. Status: adopted.

## Decision

`sub` owns the ACP bridge child handle and its dedicated process group for the lifetime of a prompt turn. When a harness does not finish within five seconds after `session/cancel`, the ACP client marks force termination as required, sends `SIGKILL` to the owned process group, terminates the direct child as a fallback, and waits for that child to exit before returning the cancelled outcome to the supervisor. Only then may the supervisor write the cancelled result and terminal events. `attempt_cancelled { harness_honored: false }` continues to distinguish forced termination from protocol-honored cancellation.

Normal prompt completion closes the protocol transport and allows the bridge one second to exit before applying the same owned-group cleanup. Dropping an in-flight client also signals only its owned process group. The fake ignore-cancel scenario can write its PID to a caller-supplied temporary marker, and the supervisor test verifies that this exact PID/start identity is dead when the cancelled result is available.

## Rationale

An ACP notification is advisory and an ignored notification cannot establish terminal cancellation. Retaining the process handle lets `sub` turn the mental model's terminal-cancel guarantee into an ordering rule: force and reap first, publish second. A dedicated process group reaches wrapper descendants without signaling unrelated processes.

## Revisit when

ACP defines a portable acknowledged-and-terminated operation, supported harnesses require a cleanup signal before `SIGKILL`, or Windows process-tree ownership enters the release scope.
