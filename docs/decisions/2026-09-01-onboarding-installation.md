# Onboarding installation for Claude and Codex

Date: 2026-09-01. Status: adopted.

## Decision

`sub onboard <claude|codex>...` is the explicit onboarding command. It accepts one or more named harnesses, rejects any that lack a `sub.toml` entry before writing, deduplicates repeated names, and never visits an unnamed harness. For each requested harness it verifies or repairs the pinned bridge in the selected state directory, writes the same `sub-delegation` skill content in that harness's skill format, and upserts a user-level stdio MCP server named `sub` whose command is the `sub-mcp` executable beside `sub`. Output is a JSON array in request order; each element has `harness` and `bridge`, `skill`, and `mcp` actions, and each action has `status` (`installed`, `created`, `updated`, or `unchanged`) and `path`.

Claude installation writes the skill to `~/.claude/skills/sub-delegation/SKILL.md` and the MCP entry under `mcpServers.sub` in `~/.claude.json`. Codex installation writes the skill to `${CODEX_HOME:-~/.codex}/skills/sub-delegation/SKILL.md` and the MCP entry under `mcp_servers.sub` in `${CODEX_HOME:-~/.codex}/config.toml`. The writers preserve unrelated JSON/TOML values. `SUB_CLAUDE_CONFIG`, `SUB_CLAUDE_SKILLS_DIR`, `SUB_CODEX_CONFIG`, and `SUB_CODEX_SKILLS_DIR` override every destination; `SUB_MCP_BINARY` overrides the registered executable for tests. Repository tests and captured scenarios set all of these and `SUB_CONFIG`, so development never modifies real harness configuration.

The installed skill tells the manager when delegation is useful; directs it through `sub_launch`, repeatable `sub_wait`, `sub_list`, `sub_inspect`, `sub_cancel`, and `sub_recover`; prefers bounded results and artifact references over transcript reconstruction; and requires bounded child tasks with no child subagents. Onboarding passes no credentials and relies on each harness's existing authentication at launch.

## Rationale

One explicit command completes the three setup steps required by the mental model without hiding changes to unrelated harnesses. Direct, format-aware upserts make the operation deterministic and testable under throwaway roots, while component-level statuses make repair and no-op reruns visible. One shared skill keeps the product guidance consistent across the two beta-path manager harnesses.

## Revisit when

Claude or Codex changes its user skill or MCP schema, `cursor-agent` becomes a working launch adapter, the binaries are distributed separately, or a harness supplies a safer supported registration API that can target an isolated configuration root.
