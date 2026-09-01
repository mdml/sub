# Observe event vocabulary and usage accumulation

Date: 2026-09-01. Status: adopted.

## Decision

The supervisor appends one typed JSON object per line to the existing attempt `events.jsonl`; Observe does not create a second log. Every record carries its timestamp, task handle, and attempt number. The event kinds are:

| Kind | Meaning |
|:--|:--|
| `task_created` | Links a delegated task to its first execution attempt. |
| `attempt_started` | A supervisor began a new harness invocation. |
| `attempt_resumed` | A replacement attempt resumed a vendor session; reserved until Recover emits it. |
| `attempt_cancelled` | The harness confirmed cancellation. |
| `attempt_finished` | An attempt reached a terminal `succeeded`, `failed`, or `cancelled` status. |
| `activity` | A coalesced category—message, thought, tool call, tool update, plan, session metadata, available commands, denied permission, observed subagent, or other—arrived from the ACP stream. It contains no message text, thought text, tool output, diff, or other transcript content. |
| `usage_accumulated` | A reported cost or per-turn token total changed; the event contains current attempt and task totals. |

The vendor's native session record remains raw evidence by reference. The normalized event log records only the timing, linkage, lifecycle, activity categories, and accounting that `sub` needs to make delegated work independently observable.

`sub-sdk` alone enables the ACP Rust SDK's `unstable_end_turn_token_usage` feature. A prompt response's per-turn usage is added when the turn ends. A streamed ACP `usage_update.cost` is a cumulative session amount, so `sub` replaces the attempt's latest cost snapshot instead of summing snapshots; task accumulation sums final attempt values when more than one attempt exists. The supervisor publishes usage to durable attempt state and appends `usage_accumulated` as reports arrive. Missing measurements remain `null`, never zero or estimated.

Observers receive `usage_support` beside each task's `usage`. Verified support is Claude Code: cost and tokens; Codex: tokens but no cost; Cursor Agent: neither. A `false` support field makes a null value mean “not reported by this harness.” A supported field may remain null while an attempt is running or if a bridge unexpectedly omits its report.

The read-only SDK shapes are `TaskList`, `TaskOverview`, `TaskInspection`, `AttemptObservation`, `TaskEvent`, `TaskEventKind`, `ActivityKind`, `UsageSupport`, `UsageTotals`, and `UsageCost`. `Delegator::list` and `Delegator::inspect` read only the configured state directory and never contact a supervisor, bridge, or harness. CLI `sub list` and `sub inspect HANDLE` and MCP `sub_list` and `sub_inspect` serialize those same shapes.

## Rationale

A typed, content-free log keeps the vendor transcript as the sole raw conversation record while preserving the facts only `sub` can state durably. Per-turn tokens avoid mistaking ACP context-window updates for spend, and replacement of cumulative cost avoids double counting. Shared SDK response types make CLI and MCP observations semantically identical and allow an unrelated process with the state directory to inspect work during or after execution.

## Revisit when

ACP stabilizes per-turn usage, a harness changes its reporting support, retries or parallel attempts require mixed currencies or partial totals, Recover begins emitting `attempt_resumed`, or activity coalescing no longer provides enough live signal.
