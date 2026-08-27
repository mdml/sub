# Workspace layout

Date: 2026-08-27. Status: adopted.

## Decision

One Cargo workspace with seven crates under `crates/`: `sub-sdk` (kernel), `sub-cli` and `sub-mcp` (surfaces), `sub-harness-fake`, and one `sub-adapter-<harness>` crate per first-release harness (`claude`, `codex`, `cursor`). Dependency versions and lints are set once at the workspace level. The `sub-cli` package builds the `sub` binary.

## Rationale

The mental model makes the SDK the product kernel and has the MCP and CLI surfaces consume it, so the layout mirrors that dependency direction and lets the compiler enforce it: a surface cannot reach a harness protocol except through the SDK. Separate adapter crates let each adapter declare the harness and bridge versions it was verified against in its own manifest and be tested in isolation against the fake harness, and a separate fake-harness crate keeps test scaffolding out of the kernel's dependency graph. Seven small crates cost little at this size and avoid a later split; the alternative, one crate with modules, would leave the surface/kernel boundary as a convention only. A TUI crate is deliberately absent: the mental model makes the TUI optional and gated on beta usage.

## Revisit when

An adapter needs code shared with another adapter (add a `sub-adapter-common` crate rather than growing the SDK), or the TUI is decided.
