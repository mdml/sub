# Nightly: mental-model audit

Purpose: find drift between the owner's mental model and the repository, in either direction, and report it as candidate changes for the owner. The audit never edits the mental model.

## Job definition

1. Start a headless `claude` session in `auto` mode with the `sub-mental-model` skill and Dogtag account connector available, in a fresh detached worktree of the local `staging` ref.
2. Invoke the skill for its operating rules, but read the human-owned mental model itself in full through the Dogtag MCP `show` tool. If the connector remains unavailable after a bounded retry, stop as blocked; never fall back to the vault checkout on disk.
3. Inspect the repository's canonical product, model, architecture, decision, and current-work documents, plus the manifests, source, and tests required for the current phase. Resolved spike reports are historical evidence rather than current decisions; derived notes are not co-equal sources of truth.
4. Report only material discrepancies in four classes: missing from the owner's model, contradicted by the repository, unrepresented decision, and vocabulary mismatch. For each discrepancy, cite repository evidence, explain why it matters to current work, and state the smallest owner decision required in the mental model's vocabulary. Do not prescribe owner-reserved decisions.
5. The script writes the returned Markdown to a dated file in a local directory outside the repository. Reports can quote the private mental model, so they are never committed or attached to a pull request. Each report records the audited commit, timestamps, agent duration and cost, harness session ID, and placeholders for owner review duration and false-positive classification.
6. Exit non-zero when the audit is blocked or reports a material discrepancy so the scheduler surfaces the run for review. Never modify the mental model or repository.

## Scheduling

Run `scripts/nightly/mental-model-audit.sh` once a day from the owner's scheduler. Set `SUB_AUDIT_ENABLED=1`; reports default to `~/.local/state/sub-audit` and can be redirected with `SUB_AUDIT_REPORT_DIR`. The job requires `claude`, `jq`, the user-level `sub-mental-model` skill, and an authenticated Dogtag account connector.

## Status

Enabled on the owner's Linux machine through a user-level systemd timer scheduled for 05:00 America/New_York. The timer has persistent catch-up enabled after downtime. The script remains disabled for unscheduled invocations unless `SUB_AUDIT_ENABLED=1` is set explicitly.
