# Developing with `claude`, `codex`, and `cursor-agent`

All three harnesses develop this repository with the same instructions, the same verification entry point, and checked-in permission configuration. Rationale: [`decisions/2026-08-27-harness-configuration.md`](decisions/2026-08-27-harness-configuration.md).

## Instructions

`AGENTS.md` is the single source. `CLAUDE.md` contains only `@AGENTS.md`; `codex` and `cursor-agent` read `AGENTS.md` natively. Do not add instructions anywhere else.

## The mental model

Reaches every harness as the user-level `sub-mental-model` skill, maintained outside this repository (the mental model says where). Nothing in this repository points at its path.

## Per-harness configuration

| Harness | File | Content |
|:--|:--|:--|
| `claude` | `.claude/settings.json` | Permission allow-list for the verification commands (`scripts/verify.sh`, `just`, `cargo`, read-only `git` and `gh`). |
| `codex` | `.codex/config.toml` | Project-scoped config. Currently only a comment; present so project-level overrides have one place. |
| `cursor-agent` | `.cursor/cli.json` | Permission allow-list equivalent to the `claude` one. |

Nothing in these files is machine-specific or secret. User-level configuration (MCP servers, skills, auth) is each developer's own and is not checked in.

## Verification

Every harness runs `scripts/verify.sh` (or `just verify`) before each commit. See [`verification.md`](verification.md).
