# Docs skeleton

Date: 2026-08-27. Status: adopted.

## Decision

`docs/README.md` is the index. Under it: `architecture.md`, `verification.md`, `harnesses.md`, `release.md` (current-state documents), `decisions/` (dated records, one per decision, indexed in `decisions/README.md`), `nightlies/` (project-specific compatibility job definitions), and `spikes/` (reports; evidence under `spikes/<name>/` at the repository root). Current-state documents are rewritten when the state changes; records are appended, never rewritten.

## Rationale

Repository documentation must describe the current state and never duplicate the mental model, so the skeleton separates the two kinds of writing that have different rules: current-state pages that are kept consistent with code in the same PR, and records (decisions, spikes) that capture what was thought at a date. The index keeps every page one link from the root. A doc that needs an owner decision names the mental model, never its path, because the path is machine-specific and the public repository must carry nothing owner-specific.

## Revisit when

A user-facing manual is needed (add `docs/guide/`, kept separate from the developer documents).
