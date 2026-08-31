#!/usr/bin/env sh
# On-machine nightly: mental-model audit. Definition: docs/nightlies/mental-model-audit.md
# Usage: scripts/nightly/mental-model-audit.sh
# Schedule this from the owner's scheduler; it is not run on GitHub.
set -eu
umask 077
REPO_ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
REPORT_DIR="${SUB_AUDIT_REPORT_DIR:-$HOME/.local/state/sub-audit}"
PROMPT='Run the deliberate nightly mental-model audit for `sub`.

Invoke the `sub-mental-model` skill for its operating rules. Read the human-owned mental model itself in full only through the Dogtag MCP `show` tool for `2026-08-24_note_sub-mental-model` in the `documents` layer; do not read the vault checkout from disk. If the Dogtag connector is still connecting, wait up to 30 seconds and retry. If it remains unavailable, report the audit as blocked instead of falling back to another copy.

Begin with a repository-consistency preflight. Read the relevant canonical product, model, architecture, decision, and current-work documents first as the intended description of the repository current state. Treat resolved spike reports as historical evidence, not current decisions, and do not treat derived notes as co-equal sources of truth. Then verify the canonical documentation claims against the manifests, source, tests, schemas, and CI needed to evaluate the current phase.

Report material documentation-versus-implementation discrepancies separately under `Repository documentation drift`. For each, state the documented claim, the conflicting implementation evidence with file and line references, why the mismatch matters, and the smallest repository correction or clarification needed. Do not infer product intent from implementation, silently choose the documentation or code as correct, or fix anything. A repository-drift finding is not automatically a mental-model finding. If unresolved repository drift makes a mental-model comparison ambiguous, state the comparison conditionally and ask for the smallest clarification instead of choosing a side.

After the repository-consistency preflight, compare the documented and verified repository state with the mental model. Report only material mental-model discrepancies in these four classes:

1. Missing from my model — an important repository concept or relationship required by current work that the mental model does not contain.
2. Contradicted by the repository — a mental-model claim that the maintained repository no longer supports.
3. Unrepresented decision — a consequential implemented product, domain, data-shape, public-interface, abstraction-boundary, or difficult-to-reverse decision absent from the mental model.
4. Vocabulary mismatch — the same term or relationship means something materially different in the mental model and repository.

For each discrepancy, state the repository evidence with file and line references, why the gap matters to current work, and the smallest decision the owner needs to make. Translate technical questions into the mental-model vocabulary. Do not prescribe an owner-reserved decision. If a class has no discrepancies, say none.

Do not modify the mental model or repository, create commits or pull requests, or delegate to subagents. Return the complete report as Markdown in the final response. End with exactly these two lines, using `yes` or `no` for each:

`REPOSITORY_DRIFT: yes|no`
`MENTAL_MODEL_FINDINGS: yes|no`

If the required source or evidence is unavailable, end with exactly `AUDIT: blocked` instead.'

if [ "${SUB_AUDIT_ENABLED:-0}" != "1" ]; then
    echo "mental-model-audit: disabled. Would run 'claude' in auto mode with this prompt:" >&2
    printf '\n%s\n\n' "$PROMPT" >&2
    echo "mental-model-audit: set SUB_AUDIT_ENABLED=1 to enable." >&2
    exit 1
fi

mkdir -p "$REPORT_DIR"
command -v claude >/dev/null 2>&1 || {
    echo "mental-model-audit: claude is not on PATH" >&2
    exit 2
}
command -v jq >/dev/null 2>&1 || {
    echo "mental-model-audit: jq is not on PATH" >&2
    exit 2
}

AUDIT_PARENT="$(mktemp -d "${TMPDIR:-/tmp}/sub-mental-model-audit.XXXXXX")"
AUDIT_WORKTREE="$AUDIT_PARENT/worktree"
cleanup() {
    git -C "$REPO_ROOT" worktree remove --force "$AUDIT_WORKTREE" >/dev/null 2>&1 || true
    rmdir "$AUDIT_PARENT" >/dev/null 2>&1 || true
}
trap cleanup EXIT HUP INT TERM

git -C "$REPO_ROOT" worktree add --detach "$AUDIT_WORKTREE" staging >/dev/null
COMMIT="$(git -C "$AUDIT_WORKTREE" rev-parse HEAD)"
STARTED_AT="$(date --iso-8601=seconds)"

set +e
RAW="$(cd "$AUDIT_WORKTREE" && claude --permission-mode auto --disallowedTools "Write,Edit,NotebookEdit" --output-format json -p "$PROMPT")"
CLAUDE_STATUS=$?
set -e

ENDED_AT="$(date --iso-8601=seconds)"
REPORT_PATH="$REPORT_DIR/$(date +%F).md"

if ! printf '%s' "$RAW" | jq -e '.type == "result"' >/dev/null 2>&1; then
    {
        printf '# `sub` mental-model audit — %s\n\n' "$(date +%F)"
        printf -- '- Repository commit: `%s`\n' "$COMMIT"
        printf -- '- Started: %s\n' "$STARTED_AT"
        printf -- '- Finished: %s\n' "$ENDED_AT"
        printf -- '- Claude exit status: %s\n\n' "$CLAUDE_STATUS"
        printf 'The Claude runner did not return a result envelope.\n\n'
        printf '```text\n%s\n```\n\nAUDIT: blocked\n' "$RAW"
    } >"$REPORT_PATH"
    printf '%s\n' "mental-model-audit: blocked; report: $REPORT_PATH" >&2
    exit 2
fi

RESULT="$(printf '%s' "$RAW" | jq -r '.result // ""')"
DURATION_MS="$(printf '%s' "$RAW" | jq -r '.duration_ms // "unknown"')"
COST_USD="$(printf '%s' "$RAW" | jq -r 'if .total_cost_usd == null then "unknown" else ((.total_cost_usd * 1000000 | round) / 1000000 | tostring) end')"
SESSION_ID="$(printf '%s' "$RAW" | jq -r '.session_id // "unknown"')"

{
    printf '# `sub` mental-model audit — %s\n\n' "$(date +%F)"
    printf -- '- Repository commit: `%s`\n' "$COMMIT"
    printf -- '- Started: %s\n' "$STARTED_AT"
    printf -- '- Finished: %s\n' "$ENDED_AT"
    printf -- '- Agent duration: %s ms\n' "$DURATION_MS"
    printf -- '- Agent cost: USD %s\n' "$COST_USD"
    printf -- '- Harness session: `%s`\n' "$SESSION_ID"
    printf -- '- Owner review duration: pending\n'
    printf -- '- False positives: pending owner review\n'
    printf -- '- Disposition: pending owner review\n\n'
    printf '%s\n' "$RESULT"
} >"$REPORT_PATH"

printf '%s\n' "$RESULT"
printf '%s\n' "mental-model-audit: report: $REPORT_PATH"

if [ "$CLAUDE_STATUS" -ne 0 ] || printf '%s\n' "$RESULT" | grep -qx 'AUDIT: blocked'; then
    exit 2
fi
# Exit non-zero on findings so the scheduler surfaces them.
if printf '%s\n' "$RESULT" | grep -qx 'REPOSITORY_DRIFT: yes' || printf '%s\n' "$RESULT" | grep -qx 'MENTAL_MODEL_FINDINGS: yes'; then
    exit 1
fi
printf '%s\n' "$RESULT" | grep -qx 'REPOSITORY_DRIFT: no' || exit 2
printf '%s\n' "$RESULT" | grep -qx 'MENTAL_MODEL_FINDINGS: no' || exit 2
exit 0
