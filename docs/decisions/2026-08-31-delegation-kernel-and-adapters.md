# Delegation kernel, durable handles, results, and adapter side channels

Date: 2026-08-31. Status: superseded in part on 2026-09-02 by [Portable Unix supervisor detachment and identity](2026-09-02-portable-unix-supervision.md); the kernel and adapter boundaries remain adopted.

## Decision

The beta kernel stores each delegated task under `tasks/<task handle>/attempts/<number>/` inside the configured state directory. Each attempt directory contains a serialized supervisor request, lifecycle state, an append-only normalized event log, supervisor diagnostics, and any terminal result. Launch creates attempt 1; explicit recovery may create later sequential attempts. This layout is implementation-private before 1.0; callers use only the task handle and SDK, CLI, or MCP controls.

A task handle has the opaque form `tsk_<24 lowercase hex characters>`. It identifies the delegated task rather than the harness session, does not encode filesystem locations or vendor identity, and remains stable when a replacement caller waits on or recovers the task. Attempts are numbered sequentially within that stable task identity.

Launch validates the working directory and user-owned harness binary, writes queued state, and starts a per-attempt supervisor. On Linux the supervisor is placed in a new session with `setsid -f`; on other supported Unix systems the child process is independent because it has null stdin and file-backed stdout and stderr. The supervisor owns the ACP bridge connection, writes running state and stream events, derives a terminal result, and atomically publishes that result. Wait follows the latest sequential attempt until its caller-supplied timeout, returning `{state: "running", status}`, `{state: "orphaned", status}`, or `{state: "complete", result}`; it does not consume the result.

Results map ACP `end_turn` to `succeeded`, `cancelled` to `cancelled`, and refusal or limit stop reasons to `failed`. Summary is the concatenated final assistant-message stream. Changed files are the sorted union of edit/delete/move tool locations and existing files linked from the final streamed Markdown within the supplied working directory; the Markdown fallback is necessary because Codex can create a file through an execute tool without attaching an ACP edit location. Artifacts reference the normalized event log, supervisor log, and harness-native session record. The native record is located by the harness session ID and retained only as a path or stable harness/session locator; its content is never copied into `sub` state or treated as the result.

The Claude adapter pins `@agentclientprotocol/claude-agent-acp` 0.70.0, sets `CLAUDE_CODE_EXECUTABLE` to the launch parameter's harness binary, clears inherited `CLAUDECODE`, sends `_meta.claudeCode.options.disallowedTools = ["Task", "Agent"]`, and applies the requested permission mode and optional model through the shared ACP client. The Codex adapter pins `@agentclientprotocol/codex-acp` 1.6.2, sets `CODEX_PATH` to the launch parameter's harness binary, supplies `CODEX_CONFIG={"features":{"multi_agent":false}}`, applies the requested permission mode and optional model through the shared ACP client, and adds the no-subagents prompt guard because the config switch's enforcement remains unverified. Both adapters add the prompt guard, and the shared client denies and records every residual permission request.

## Rationale

Separating task identity from vendor session identity lets a manager exit and a replacement caller use the same bounded handle without exposing the private state layout. Atomic result publication and repeatable wait avoid transcript reconstruction. Keeping bridge-specific environment and `_meta` values in adapter crates preserves harness-native behavior while all ACP transport remains in `sub-sdk`. Stream-derived results keep the manager handoff small and make the vendor transcript evidence rather than an accidental second result format.

## Revisit when

Retries or parallel attempts enter scope, ACP v2 changes prompt completion, a bridge supplies a reliable final changed-file report, Linux process-session detachment proves insufficient, or the private state layout needs migration/versioning guarantees.
