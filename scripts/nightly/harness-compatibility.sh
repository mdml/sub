#!/usr/bin/env sh
# On-machine nightly: harness compatibility. Definition: docs/nightlies/harness-compatibility.md
# Schedule this from the owner's scheduler; it is not run on GitHub.
set -eu
cd "$(dirname "$0")/../.."

REPORT_DIR="${SUB_NIGHTLY_REPORT_DIR:-$HOME/.sub/nightlies/harness-compatibility}"
STAMP="$(date +%Y-%m-%d)"
REPORT="$REPORT_DIR/$STAMP.txt"
FAIL=0

mkdir -p "$REPORT_DIR"
: >"$REPORT"

log() {
    printf '%s\n' "$*" | tee -a "$REPORT"
}

log "harness-compatibility: installed versions"
for h in claude codex cursor-agent; do
    if command -v "$h" >/dev/null 2>&1; then
        line="$(printf '  %s: %s' "$h" "$("$h" --version 2>/dev/null | head -n 1)")"
    else
        line="$(printf '  %s: not installed' "$h")"
    fi
    log "$line"
done

log ""
log "harness-compatibility: adapter declared versions (stubs until item 3)"
log "  sub-adapter-claude: not declared yet"
log "  sub-adapter-codex: not declared yet"
log "  sub-adapter-cursor: not declared yet"
log "  version mismatches are reported only; adapters do not exist yet"

log ""
log "harness-compatibility: behavioral contract suite (real-harness mode)"

if ! cargo build -p sub-harness-fake --locked >/dev/null 2>&1; then
    log "  failed to build sub-harness-fake; aborting contract suite"
    exit 1
fi

TMPDIR="${TMPDIR:-/tmp}"
WORK="$TMPDIR/sub-harness-compat-$STAMP-$$"
mkdir -p "$WORK"

run_real() {
    name=$1
    if ! command -v "$name" >/dev/null 2>&1; then
        log "  $name: skipped (not installed)"
        return 0
    fi
    log "  $name: running contract suite"
    if (
        cd "$WORK" &&
            SUB_CONTRACT_REAL_HARNESS="$name" \
            cargo test -p sub-sdk --test behavioral_contract real_harness_mode_entrypoint --locked -- --nocapture
    ); then
        log "  $name: pass"
    else
        log "  $name: FAIL (a test that passes on the fake harness and fails here means the fake is wrong)"
        FAIL=1
    fi
}

run_real claude
run_real codex
run_real cursor-agent

log ""
if [ "$FAIL" -eq 0 ]; then
    log "harness-compatibility: OK (report: $REPORT)"
else
    log "harness-compatibility: failed (report: $REPORT)"
fi
exit "$FAIL"
