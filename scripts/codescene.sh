#!/usr/bin/env sh
# CodeScene code-health check for the full gate.
#
# Gate: every eligible file (a file CodeScene scores at all) has code health 10.
#
# Requires the CS_ACCESS_TOKEN secret (a CodeScene access token, created by a
# CodeScene administrator on the project's configuration page). When it is
# absent this script fails and names the secret, so the full gate cannot pass
# without it.
#
# The CodeScene CLI (`cs`) is used from PATH if present; otherwise the official
# installer is run with HOME pointed at ./target/codescene so nothing lands
# outside the repository. Installer:
# https://downloads.codescene.io/enterprise/cli/install-cs-tool.sh
#
# The CLI has no "fail below threshold" flag (command reference, 2026-08-27),
# so this script reads `cs review --output-format json` and applies the
# threshold itself. The JSON field carrying the score is read as `score`; if
# the CLI reports a different field the script fails loudly and prints the
# output so the field name can be corrected here.
set -eu

cd "$(dirname "$0")/.."

if [ -z "${CS_ACCESS_TOKEN:-}" ]; then
    echo "codescene: CS_ACCESS_TOKEN is not set. Add the CodeScene access token as the" >&2
    echo "codescene: CS_ACCESS_TOKEN repository secret (GitHub Actions) or export it locally." >&2
    exit 3
fi

if command -v cs >/dev/null 2>&1; then
    CS=cs
else
    CS_HOME="$PWD/target/codescene"
    CS="$CS_HOME/.local/bin/cs"
    if [ ! -x "$CS" ]; then
        mkdir -p "$CS_HOME"
        echo "codescene: installing the CodeScene CLI under $CS_HOME"
        curl -fsSL https://downloads.codescene.io/enterprise/cli/install-cs-tool.sh | HOME="$CS_HOME" sh
        CS="$(find "$CS_HOME" -type f -name cs -perm -u+x | head -n 1)"
        [ -n "$CS" ] || { echo "codescene: installer did not produce a 'cs' binary under $CS_HOME" >&2; exit 3; }
    fi
fi

status=0
for f in $(git ls-files '*.rs'); do
    out="$("$CS" review --output-format json "$f")" || { echo "codescene: review failed for $f" >&2; status=1; continue; }
    score="$(printf '%s' "$out" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("score", ""))')"
    if [ -z "$score" ]; then
        echo "codescene: no 'score' field for $f; adjust scripts/codescene.sh. Output was:" >&2
        printf '%s\n' "$out" >&2
        status=1
    elif [ "$(printf '%s' "$score" | awk '{print ($1 < 10) ? 1 : 0}')" = 1 ]; then
        echo "codescene: $f scored $score (< 10)" >&2
        status=1
    else
        echo "codescene: $f scored $score"
    fi
done
exit $status
