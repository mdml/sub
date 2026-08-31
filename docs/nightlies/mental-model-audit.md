# Nightly: mental-model audit

Purpose: find internal drift between the repository's canonical documentation and implementation, then find drift between the owner’s mental model and the documented-and-verified repository state. The audit reports candidate dispositions but never edits the mental model or repository.

## Job definition

1. Start a headless `claude` session in `auto` mode with the `sub-mental-model` skill and Dogtag account connector available, in a fresh detached worktree of the local `staging` ref.
2. Invoke the skill for its operating rules, but read the human-owned mental model itself in full through the Dogtag MCP `show` tool. If the connector remains unavailable after a bounded retry, stop as blocked; never fall back to the vault checkout on disk.
3. Run one repository-consistency preflight before the mental-model comparison. Read relevant canonical product, model, architecture, decision, and current-work documents first as the intended current state, then verify their claims against the manifests, source, tests, schemas, and CI required for the current phase. Resolved spike reports are historical evidence rather than current decisions; derived notes are not co-equal sources of truth.
4. Report material documentation-versus-implementation discrepancies separately as repository documentation drift. State both claims and the smallest repository correction or clarification; never infer product intent from implementation or silently choose one side. Repository drift is not automatically a mental-model discrepancy, and an ambiguous comparison remains conditional until the repository inconsistency is resolved.
5. Compare the documented-and-verified repository state with the mental model and report only material discrepancies in four classes: missing from the owner's model, contradicted by the repository, unrepresented decision, and vocabulary mismatch. For each discrepancy, cite repository evidence, explain why it matters to current work, and state the smallest owner decision required in the mental model's vocabulary. Do not prescribe owner-reserved decisions.
6. The script writes the returned Markdown to a dated file in a local directory outside the repository. Reports can quote the private mental model, so they are never committed or attached to a pull request. Each report records the audited commit, timestamps, agent duration and cost, harness session ID, and placeholders for owner review duration, false-positive classification, and disposition.
7. Exit non-zero when the audit is blocked, reports repository documentation drift, or reports a mental-model discrepancy so the scheduler surfaces the run for review. Never modify the mental model or repository, open a pull request, or delegate fixes.

## Disposition

- Repository documentation drift is a false positive, needs a clarification, or needs a repository correction. A genuine correction follows the ordinary repository process: a separate worktree and PR, rebase-merged into `staging`. Opening the PR leaves the finding pending; merge into the audited `staging` branch disposes it. Promotion from `staging` to `main` remains proof-gated and is not required to close the nightly finding.
- A mental-model discrepancy returns to the owner for one of three dispositions: update the human-owned mental model, correct the repository through a separate PR rebase-merged into `staging`, or consciously accept and record the mismatch in the stable source that governs it. If the owner decision requires both model and repository changes, make them separately; the nightly report substitutes for neither.
- Record the disposition and owner review time in the private report. A later nightly independently verifies that the maintained sources no longer produce the finding.

## Scheduling

Run `scripts/nightly/mental-model-audit.sh` once a day from the owner's scheduler. Set `SUB_AUDIT_ENABLED=1`; reports default to `~/.local/state/sub-audit` and can be redirected with `SUB_AUDIT_REPORT_DIR`. The job requires `claude`, `jq`, the user-level `sub-mental-model` skill, and an authenticated Dogtag account connector.

## Status

Enabled on the owner's Linux machine through a user-level systemd timer scheduled for 05:00 America/New_York. The timer has persistent catch-up enabled after downtime. The script remains disabled for unscheduled invocations unless `SUB_AUDIT_ENABLED=1` is set explicitly.
