# `sub.toml` location and launch precedence

Date: 2026-09-01. Status: adopted.

## Decision

The beta configuration file is `$XDG_CONFIG_HOME/sub/sub.toml`, falling back to `$HOME/.config/sub/sub.toml` when `XDG_CONFIG_HOME` is unset. `SUB_CONFIG` selects one exact alternative file for tests and isolated scenarios. A missing discovered file is an empty configuration; an existing malformed file is an error. The schema rejects unknown fields and contains only optional `state_dir` plus `harnesses.claude` and `harnesses.codex` entries. Each harness entry requires `binary` and may set `model` and `permission_mode`.

For CLI and MCP launch, an explicit argument overrides the corresponding harness entry. Omitted binary, model, and permission mode values come from that entry. Binary and permission mode remain required after resolution; `sub` never guesses an executable from `PATH` or supplies a harness-native permission mode. For state selection, an explicit CLI/MCP state directory overrides `sub.toml`; configured state overrides the retained environment and implementation defaults. The resolved values enter the existing `LaunchParams`, so configuration does not create another launch path or change task semantics.

## Rationale

The XDG configuration location is a conventional user-level, inspectable location and keeps configuration separate from the implementation-private state layout. An exact path override makes discovery testable without touching the owner's files. Strict parsing prevents accidental expansion beyond the mental model's beta-minimum scope, while argument-first field resolution preserves every explicit proof command and lets callers override one configured default without restating the others.

## Revisit when

An interactive configuration command enters scope, the first release adds another working adapter, or configuration needs versioning or migration guarantees.
