# Nightly: mental-model audit

Purpose: find drift between the owner's mental model and the repository, in either direction, and report it as candidate changes for the owner. The audit never edits the mental model.

## Job definition

1. Start a harness session (any of the three) in `auto` mode with the `sub-mental-model` skill available, in a fresh worktree of `staging`.
2. The prompt asks the agent to invoke the `sub-mental-model` skill and read the mental model in full, the same as any other session, then read `AGENTS.md`, `docs/`, and the crate manifests, and to list: (a) repository statements that contradict the mental model, (b) decisions the repository has frozen that the mental model reserves, (c) mental-model claims the repository's evidence (spikes, tests) now contradicts.
3. The agent writes the report to a dated file in a local directory outside the repository. Reports quote the mental model, which is not public, so they are never committed or attached to a pull request; the owner reads them locally.
4. Exit non-zero when any finding of kind (a), (b), or (c) is reported.

## Scheduling

Run `scripts/nightly/mental-model-audit.sh` once a day from the owner's scheduler. The script takes the harness name as its first argument (default `claude`) and writes reports under `SUB_AUDIT_REPORT_DIR` (default `~/.local/state/sub-audit`).

## Status

Definition only. The script prints the prompt it would send and exits non-zero until the owner enables it by setting `SUB_AUDIT_ENABLED=1`.
