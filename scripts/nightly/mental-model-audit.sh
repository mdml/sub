#!/usr/bin/env sh
# On-machine nightly: mental-model audit. Definition: docs/nightlies/mental-model-audit.md
# Usage: scripts/nightly/mental-model-audit.sh [claude|codex|cursor-agent]
# Schedule this from the owner's scheduler; it is not run on GitHub.
set -eu
cd "$(dirname "$0")/../.."

HARNESS="${1:-claude}"
PROMPT='Audit this repository against the sub mental model. Invoke the sub-mental-model skill and read the mental model in full through the Dogtag MCP. Then read AGENTS.md, docs/, and every Cargo.toml. Report, with file and line references: (a) repository statements that contradict the mental model; (b) decisions the repository has frozen that the mental model reserves for the owner; (c) mental-model claims that repository evidence (spikes, tests) now contradicts. Never edit the mental model. Write the report as Markdown to docs/audits/<today>.md in a new branch and open a draft pull request into staging only if (a), (b), or (c) is non-empty.'

if [ "${SUB_AUDIT_ENABLED:-0}" != "1" ]; then
    echo "mental-model-audit: disabled. Would run '$HARNESS' in auto mode with this prompt:" >&2
    printf '\n%s\n\n' "$PROMPT" >&2
    echo "mental-model-audit: set SUB_AUDIT_ENABLED=1 to enable." >&2
    exit 1
fi

case "$HARNESS" in
    claude) exec claude --permission-mode auto -p "$PROMPT" ;;
    codex) exec codex exec --full-auto "$PROMPT" ;;
    cursor-agent) exec cursor-agent -p --force "$PROMPT" ;;
    *) echo "mental-model-audit: unknown harness '$HARNESS'" >&2; exit 2 ;;
esac
