#!/usr/bin/env sh
# On-machine nightly: harness compatibility. Definition: docs/nightlies/harness-compatibility.md
# Schedule this from the owner's scheduler; it is not run on GitHub.
set -eu
cd "$(dirname "$0")/../.."

echo "harness-compatibility: installed versions"
for h in claude codex cursor-agent; do
    if command -v "$h" >/dev/null 2>&1; then
        printf '  %s: %s\n' "$h" "$("$h" --version 2>/dev/null | head -n 1)"
    else
        printf '  %s: not installed\n' "$h"
    fi
done

# Steps 2-5 of the definition need the adapters' declared versions and the
# contract suite's real-harness mode, neither of which exists yet.
echo "harness-compatibility: the contract suite and adapters do not exist yet; nothing to run." >&2
exit 1
