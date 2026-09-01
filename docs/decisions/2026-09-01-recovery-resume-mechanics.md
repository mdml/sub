# Recovery attempts and bridge session resume

Date: 2026-09-01. Status: adopted.

## Decision

`Delegator::recover` accepts a task handle only when its latest attempt is orphaned. It creates the next sequential attempt under a fresh detached supervisor and returns `RecoverOutcome { handle, attempt }` immediately. CLI `sub recover HANDLE` and MCP `sub_recover` expose the same control and serialize the same response. Recovery is explicit; there is no retry policy or automatic recovery.

The initial supervisor persists the harness session ID as soon as `session/new` succeeds, before the task prompt begins. A recovery supervisor opens a fresh bridge process and reopens that exact session. The Claude adapter uses `session/resume` through `@agentclientprotocol/claude-agent-acp` 0.70.0; the Codex adapter uses `session/resume` through `@agentclientprotocol/codex-acp` 1.6.2. The adapter supplies this mechanism as `ResumeMechanism`; `SessionStart::Load` remains available in the shared ACP layer for replay-loading bridges such as Cursor Agent, which is outside this Recover proof.

After the bridge accepts resume, the new attempt appends `attempt_resumed` and sends a continuation instruction that tells the child to continue the interrupted delegated task. It never sends the original task prompt again. If the predecessor did not durably record a session ID, the new attempt fails with `attempt_resume_failed: session_record_missing`. If the bridge refuses or cannot reopen the session, it fails with `attempt_resume_failed: harness_refused`. Both cases publish a failed result and never silently start a new harness session.

Every attempt retains its own state, events, diagnostics, result, and usage. Observe sums completed and partial reported usage across sequential attempts at the task level while also returning each attempt's totals. Wait follows the latest attempt and returns its durable result.

## Rationale

The task handle names semantic work, while the harness session names vendor-owned conversation state. Keeping both identities stable across a new execution attempt preserves the child's context without pretending a killed process invocation continued. Adapter-owned resume selection keeps bridge differences out of surfaces and the delegated-work kernel.

## Revisit when

Cursor Agent enters the implemented surface, ACP v2 replaces the v1 resume/load distinction, retries or parallel attempts need a more general attempt-creation control, or bridges provide structured resume-failure categories that should replace `harness_refused`.
