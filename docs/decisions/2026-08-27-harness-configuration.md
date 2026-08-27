# Harness configuration

Date: 2026-08-27. Status: adopted.

## Decision

`AGENTS.md` is the single source of agent instructions; `CLAUDE.md` imports it and `codex` and `cursor-agent` read it natively. Project-level permission configuration is checked in for each harness: `.claude/settings.json` (permission allow-list for the verification commands), `.codex/config.toml` (project-scoped settings; no overrides yet), and `.cursor/cli.json` (permission allow-list). No project-level MCP configuration is checked in: the repository's process needs no MCP server. Skills are user-level (the `sub-mental-model` skill, per the mental model). See `docs/harnesses.md`.

## Rationale

The mental model requires the repository to be fully developable through all three harnesses with the same configuration and context. Instructions already converge on `AGENTS.md`; permissions are the one thing that otherwise differs per harness, and checking in a minimal allow-list for the verification commands lets each harness run the gate in `auto` mode without prompting or widening permissions globally. Anything machine-specific, secret, or owner-specific (MCP servers with credentials, skill paths under a home directory) stays out of the repository by the public-repository rule.

## Revisit when

`sub` itself ships an MCP server that agents should use while developing it (then a project-level MCP config that launches the workspace build is appropriate), or a harness changes its project-config format.
