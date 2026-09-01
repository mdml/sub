# Cross-process cancel request signalling

Date: 2026-09-01. Status: adopted.

## Decision

`Delegator::cancel` resolves the latest attempt from a task handle and creates an empty `cancel.request` marker inside that attempt's implementation-private state directory. The per-attempt supervisor watches this path while its ACP prompt is pending. Seeing the marker causes the supervisor to send `session/cancel` on the ACP connection it owns. Creating an existing marker is idempotent.

The SDK, CLI `sub cancel`, and MCP `sub_cancel` return `CancelOutcome { handle, attempt, delivery }` immediately. `delivery` is `delivered` when a queued or running attempt has a live supervisor, `already_finished` when state is already terminal, and `attempt_orphaned` when direct process evidence says the running supervisor is dead. No marker is written for finished or orphaned attempts because no live supervisor can act on it.

The marker layout is private before 1.0. The public contract is task-handle cancellation and its immediate delivery disposition, not a state-directory path or file protocol.

## Rationale

A durable per-attempt marker works across unrelated manager, CLI, and MCP processes without adding a daemon or exposing operating-system signals as public semantics. It scopes cancellation to one task attempt and lets a request made while the supervisor is still opening the ACP session be delivered once the session exists.

## Revisit when

A daemon enters scope, multiple concurrent attempts per task require attempt selection, non-filesystem state storage is introduced, or the private state layout gains migration guarantees.
