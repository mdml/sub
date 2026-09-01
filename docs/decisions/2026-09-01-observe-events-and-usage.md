# Observe event vocabulary and usage accumulation

Date: 2026-09-01. Status: adopted.

## Decision

The supervisor appends one typed JSON object per line to the existing attempt `events.jsonl`; Observe does not create a second log. Every record carries its timestamp, task handle, and attempt number. The event kinds are:

| Kind | Meaning |
|:--|:--|
| `task_created` | Links a delegated task to its first execution attempt. |
| `attempt_started` | A supervisor began a new harness invocation. |
| `attempt_orphaned` | Explicit recovery confirmed that the predecessor's recorded supervisor was dead. |
| `attempt_resumed` | A replacement attempt reopened the predecessor's vendor session. |
| `attempt_resume_failed` | Recovery could not reopen the session because its persisted identity was missing or the harness refused it. |
| `attempt_cancelled` | Explicit cancellation ended the attempt; `harness_honored` says whether the harness acknowledged it within the grace period. |
| `attempt_recovery_rejected` | Recovery was rejected because the task was already terminal; its reason is `cancelled` in the beta. |
| `attempt_finished` | An attempt reached a terminal `succeeded`, `failed`, or `cancelled` status. |
| `activity` | A coalesced category—message, thought, tool call, tool update, plan, session metadata, available commands, denied permission, observed subagent, or other—arrived from the ACP stream. It contains no message text, thought text, tool output, diff, or other transcript content. |
| `usage_accumulated` | A reported cost or per-turn token total changed; the event contains current attempt and task totals. |

The vendor's native session record remains raw evidence by reference. The normalized event log records only the timing, linkage, lifecycle, activity categories, and accounting that `sub` needs to make delegated work independently observable.

`sub-sdk` alone enables the ACP Rust SDK's `unstable_end_turn_token_usage` feature. A prompt response's per-turn usage is added when the turn ends. A streamed ACP `usage_update.cost` is a cumulative session amount, so `sub` replaces the attempt's latest cost snapshot instead of summing snapshots; task accumulation sums reported attempt values across sequential attempts. The supervisor publishes usage to durable attempt state and appends `usage_accumulated` as reports arrive. Missing measurements remain `null`, never zero or estimated.

Observers receive `usage_support` beside each task's `usage`. Verified support is Claude Code: cost and tokens; Codex: tokens but no cost; Cursor Agent: neither. A `false` support field makes a null value mean “not reported by this harness.” A supported field may remain null while an attempt is running or if a bridge unexpectedly omits its report.

The read-only SDK shapes are `TaskList`, `TaskOverview`, `TaskInspection`, `AttemptObservation`, `TaskEvent`, `TaskEventKind`, `ActivityKind`, `ResumeFailureReason`, `UsageSupport`, `UsageTotals`, and `UsageCost`. `Delegator::list` and `Delegator::inspect` read only the configured state directory and never contact a supervisor, bridge, or harness. They derive `orphaned` from direct supervisor liveness evidence without appending an event; explicit recovery records the transition. CLI `sub list` and `sub inspect HANDLE` and MCP `sub_list` and `sub_inspect` serialize those same shapes.

## Rationale

A typed, content-free log keeps the vendor transcript as the sole raw conversation record while preserving the facts only `sub` can state durably. Per-turn tokens avoid mistaking ACP context-window updates for spend, and replacement of cumulative cost avoids double counting. Shared SDK response types make CLI and MCP observations semantically identical and allow an unrelated process with the state directory to inspect work during or after execution.

## Revisit when

ACP stabilizes per-turn usage, a harness changes its reporting support, retries or parallel attempts require mixed currencies or partial totals, or activity coalescing no longer provides enough live signal.
