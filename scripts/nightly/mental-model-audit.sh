#!/usr/bin/env sh
# On-machine nightly: mental-model audit. Definition: docs/nightlies/mental-model-audit.md
# Usage: scripts/nightly/mental-model-audit.sh [claude|codex|cursor-agent]
# Schedule this from the owner's scheduler; it is not run on GitHub.
set -eu
cd "$(dirname "$0")/../.."

HARNESS="${1:-claude}"
REPORT_DIR="${SUB_AUDIT_REPORT_DIR:-$HOME/.local/state/sub-audit}"
PROMPT="Audit this repository against the sub mental model. Invoke the sub-mental-model skill and read the mental model in full. Then read AGENTS.md, docs/, and every Cargo.toml. Report, with file and line references: (a) repository statements that contradict the mental model; (b) decisions the repository has frozen that the mental model reserves for the owner; (c) mental-model claims that repository evidence (spikes, tests) now contradicts. Never edit the mental model. Do not modify the repository or open a pull request. Write the report as Markdown to a file named by today's date under $REPORT_DIR (create the directory if needed). End your final message with exactly FINDINGS: yes if (a), (b), or (c) is non-empty, otherwise FINDINGS: no."

if [ "${SUB_AUDIT_ENABLED:-0}" != "1" ]; then
    echo "mental-model-audit: disabled. Would run '$HARNESS' in auto mode with this prompt:" >&2
    printf '\n%s\n\n' "$PROMPT" >&2
    echo "mental-model-audit: set SUB_AUDIT_ENABLED=1 to enable." >&2
    exit 1
fi

mkdir -p "$REPORT_DIR"
case "$HARNESS" in
    claude) out="$(claude --permission-mode auto -p "$PROMPT")" ;;
    codex) out="$(codex exec --full-auto "$PROMPT")" ;;
    cursor-agent) out="$(cursor-agent -p --force "$PROMPT")" ;;
    *) echo "mental-model-audit: unknown harness '$HARNESS'" >&2; exit 2 ;;
esac
printf '%s\n' "$out"
echo "mental-model-audit: reports are under $REPORT_DIR"
# Exit non-zero on findings so the scheduler surfaces them.
printf '%s' "$out" | grep -q 'FINDINGS: yes' && exit 1
exit 0
