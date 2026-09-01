# Cancellation grace period and ignored harnesses

Date: 2026-09-01. Status: adopted.

## Decision

After observing a cancel request, the supervisor sends ACP `session/cancel` and allows the harness five seconds to finish the pending prompt with stop reason `cancelled`. An acknowledgement within that grace period produces a cancelled result and `attempt_cancelled { harness_honored: true }`.

If the harness does not answer within five seconds, the supervisor ends its ACP connection and still publishes a cancelled result. The result contains all assistant text, changed-file locations, streamed usage, artifact references, and harness session identity observed before the grace period ended. Its event is `attempt_cancelled { harness_honored: false }`, followed by `attempt_finished { status: cancelled }`. This is cancellation of delegated work, not an unbounded wait and not a claim that the harness honored the protocol request.

Cancellation is terminal. Wait returns its durable result like any completed outcome. Recover does not create another attempt for a cancelled task and appends `attempt_recovery_rejected { reason: cancelled }` before returning an error.

If the harness completes before the supervisor observes the marker, normal completion wins and a later cancel call returns `already_finished`. If completion races with a delivered ACP notification and the harness returns a non-cancelled stop reason within the grace period, the reported stop reason wins; the request was delivered but did not cancel completed work.

## Rationale

Protocol cancellation gives the child a chance to stop cleanly and preserve its native session record. A fixed short bound prevents an ignoring bridge or harness from keeping the attempt running indefinitely while the boolean event field distinguishes acknowledgement from supervisor enforcement.

## Revisit when

ACP defines acknowledged cancellation separately from prompt completion, real harness evidence requires a different bound, or configurable control policy enters scope.
