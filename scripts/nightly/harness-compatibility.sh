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
log "harness-compatibility: adapter declared versions"
for adapter in claude codex cursor; do
    log "  sub-adapter-$adapter:"
    sed -n '/pub const \(BRIDGE_PACKAGE\|BRIDGE_VERSION\|VERIFIED_HARNESS_VERSIONS\)/p' "crates/sub-adapter-$adapter/src/lib.rs" |
        sed 's/^/    /' |
        tee -a "$REPORT"
done
log "  version mismatches are reported only"

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
    command_override=""
    case "$name" in
        claude) command_override=${SUB_CONTRACT_CLAUDE_CMD:-} ;;
        codex) command_override=${SUB_CONTRACT_CODEX_CMD:-} ;;
    esac
    if [ "$name" != "cursor-agent" ] && [ -z "$command_override" ]; then
        log "  $name: skipped (set SUB_CONTRACT_$(printf '%s' "$name" | tr '[:lower:]' '[:upper:]')_CMD to the path printed by sub bridge install $name)"
        return 0
    fi
    log "  $name: running contract suite"
    if (
        cd "$WORK" &&
            SUB_CONTRACT_REAL_HARNESS="$name" \
            SUB_CONTRACT_HARNESS_CMD="$command_override" \
            CLAUDE_CODE_EXECUTABLE="$(command -v claude || true)" \
            CODEX_PATH="$(command -v codex || true)" \
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
