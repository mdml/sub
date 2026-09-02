# Load-replay observation boundary

Date: 2026-09-01. Status: adopted.

## Decision

For an adapter whose `ResumeMechanism` is `Load`, the shared ACP client treats every `session/update` and observed Cursor extension notification received before the successful `session/load` response as replayed history. Replayed history is consumed to satisfy the protocol but is not passed to the attempt observer and is not included in the continuation turn's `PromptResult`. Updates received after the load response are live attempt N+1 updates and flow through normal activity, changed-file, cost, and result derivation.

The recovery supervisor records `attempt_resumed` only after `session/load` succeeds, then sends the existing continuation instruction. A missing session record produces `attempt_resume_failed: session_record_missing`; a refused or failed load produces `attempt_resume_failed: harness_refused`. Neither case creates a fresh session. Cursor usage remains absent, but the replay boundary also prevents future or harness-specific cost and token reports replayed during load from being counted again.

## Rationale

ACP v1 `session/load` replays the existing conversation as ordinary session updates before returning, while `session/resume` does not. The load response is the only protocol boundary that distinguishes historical replay from new continuation activity without comparing transcript content. Suppressing pre-response updates prevents duplicate normalized events, changed files, result text, and usage while retaining one stable task and harness-session lineage.

## Revisit when

ACP replaces v1 load with a resume operation that marks replayed updates, Cursor changes notification ordering around `session/load`, a client must expose replay progress, or a harness emits a load-time update that is required for continuation state rather than historical display.
