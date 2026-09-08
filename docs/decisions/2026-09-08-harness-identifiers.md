# Harness identifiers are binary names

Date: 2026-09-08. Status: adopted. Supersedes in part [Cursor native ACP transport and extension handling](2026-09-01-cursor-native-acp-and-extensions.md), whose public harness name `cursor` is replaced by `cursor-agent`.

## Decision

Every public harness identifier is the name of the harness binary: `claude`, `codex`, and `cursor-agent`. The identifier is used unchanged for the `sub.toml` table key (`[harnesses.cursor-agent]`), the CLI `--harness`, `sub onboard`, and `sub bridge install` values, the MCP `harness` enum value, the serialized `Harness` value in task state and results, onboarding reports, the native-session locator prefix (`cursor-agent:<cwd>:<session-id>`), the adapter's `HARNESS_NAME`, and the `harness:cursor-agent` issue label.

Serialized task state deserializes `cursor` and `cursor_agent` as aliases of `cursor-agent`. The `~/.sub` layout is implementation-private before 1.0, so the aliases are a convenience rather than a compatibility promise: they keep `sub list`, `sub inspect`, and `sub report` working on tasks recorded by earlier nightly builds without a state migration. They may be removed at 1.0. `sub.toml` accepts only `[harnesses.cursor-agent]`; a stale `[harnesses.cursor]` table fails to parse with an unknown-field error rather than silently configuring nothing.

Internal names are unchanged where they are not public harness identifiers: the `sub-adapter-cursor` crate, the `Harness::CursorAgent` variant, the `SUB_CURSOR_CONFIG` and `SUB_CURSOR_SKILLS_DIR` isolation overrides, the `cursor/...` ACP extension methods, and vendor paths under `~/.cursor`.

## Rationale

This is the owner's decision, made on 2026-09-08 after a week of nightly mental-model audits. The mental model names the harness `cursor-agent` throughout and lists the first-release harnesses as `claude`, `codex`, and `cursor-agent`; the repository's `cursor` name was a divergence from the model, not a repository-owned choice. Using the binary name removes a second vocabulary: the identifier a user types into `sub.toml`, the CLI, and the MCP call is the same word they type to run the harness.

## Revisit when

A harness ships under more than one binary name, or the mental model changes how harnesses are identified.
