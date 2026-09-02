# Cursor native ACP transport and extension handling

Date: 2026-09-01. Status: adopted.

## Decision

The public harness name is `cursor`: CLI and MCP `harness` values, `[harnesses.cursor]`, onboarding reports, and `sub-adapter-cursor` use that name. The configured binary remains the user-owned `cursor-agent` executable. The adapter launches that binary as `cursor-agent acp`, targets ACP v1, and declares Cursor Agent 2026.08.25-3e8eec8 as verified. It declares no bridge package or bridge version. `sub bridge install cursor` and the onboarding bridge action are no-ops that report native ACP instead of creating bridge state.

The requested `permission_mode` is sent with ACP `session/set_mode`; the supported values verified for Cursor are `agent`, `plan`, and `ask`. The shared client denies and records residual `session/request_permission` requests. Cursor's blocking `cursor/ask_question` and `cursor/create_plan` extension requests are answered with `cancelled` and recorded as denied interactions, so an unattended attempt never waits for input or plan approval.

Cursor has no native subagent-disable switch. The adapter appends the same no-subagents delegation guard used by the other adapters. Tool activity whose title identifies Agent, Task, subagent, or spawn-agent behavior produces `subagent_observed`; Cursor's `cursor/task` completion notification also produces that activity without copying its payload. Other Cursor extension notifications remain outside the normalized event vocabulary.

Cursor reports neither per-turn tokens nor cumulative cost through ACP. Observe therefore returns `usage_support { cost: false, tokens: false }`, and both usage values remain absent. Its native session artifact is the matching directory under `~/.cursor/acp-sessions/<session-id>` when present, otherwise the stable `cursor:<cwd>:<session-id>` locator.

Onboarding writes `sub-delegation/SKILL.md` under `~/.cursor/skills` and upserts `mcpServers.sub` in `~/.cursor/mcp.json`. `SUB_CURSOR_SKILLS_DIR` and `SUB_CURSOR_CONFIG` replace those destinations for tests and isolated scenarios.

Current-reality divergence from the 2026-08-26 spike: the captured spike stream did not establish Cursor extension handling. Cursor's current [ACP documentation](https://prod.cursor.com/docs/cli/acp) specifies the two blocking requests and the `cursor/task` subagent notification above, so the adapter handles them explicitly. The current [skill locations](https://prod.cursor.com/docs/skills) and [global MCP file](https://prod.cursor.com/help/customization/mcp) also establish the onboarding destinations; these are current vendor behavior rather than claims inferred from the spike transcript.

## Rationale

Cursor Agent supplies the ACP server directly, so installing or naming a bridge would misrepresent the launch path. Prompt enforcement plus explicit observation is the mental model's required fallback when a harness has no subagent switch. Handling Cursor's two blocking extensions preserves the unattended permission boundary, while the `cursor/task` notification is the harness's direct subagent signal and can be normalized without retaining extension content.

## Revisit when

Cursor adds a subagent-disable switch, changes its user skill or MCP locations, reports usage, changes its ACP mode identifiers, removes or standardizes its extension methods, or no longer stores ACP sessions under `~/.cursor/acp-sessions`.
