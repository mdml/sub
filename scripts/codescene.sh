#!/usr/bin/env sh
# CodeScene code-health check.
#
# Gate: every checked eligible file has code health 10.
#
# Modes:
#   scripts/codescene.sh                  whole tracked tree
#   scripts/codescene.sh --all            whole tracked tree
#   scripts/codescene.sh --staged         files staged for the next commit
#   scripts/codescene.sh --commit [REF]   files changed by REF (default: HEAD)
#   scripts/codescene.sh --base REF       files changed from REF through HEAD
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
        [ -n "$CS" ] || {
            echo "codescene: installer did not produce a 'cs' binary under $CS_HOME" >&2
            exit 3
        }
    fi
fi

mkdir -p target/codescene
FILES="$(mktemp "$PWD/target/codescene/files.XXXXXX")"
trap 'rm -f "$FILES"' EXIT HUP INT TERM

mode="${1:---all}"
case "$mode" in
    --all)
        [ "$#" -eq 0 ] || [ "$#" -eq 1 ] || {
            echo "codescene: --all accepts no argument" >&2
            exit 2
        }
        git ls-files '*.rs' >"$FILES"
        ;;
    --staged)
        [ "$#" -eq 1 ] || {
            echo "codescene: --staged accepts no argument" >&2
            exit 2
        }
        git diff --cached --name-only --diff-filter=ACMR -- '*.rs' >"$FILES"
        ;;
    --commit)
        [ "$#" -le 2 ] || {
            echo "codescene: --commit accepts at most one ref" >&2
            exit 2
        }
        ref="${2:-HEAD}"
        git rev-parse --verify "$ref^{commit}" >/dev/null
        git diff-tree --no-commit-id --name-only -r --diff-filter=ACMR "$ref" -- '*.rs' >"$FILES"
        ;;
    --base)
        [ "$#" -eq 2 ] || {
            echo "codescene: --base requires a base ref" >&2
            exit 2
        }
        git rev-parse --verify "$2^{commit}" >/dev/null
        git diff --name-only --diff-filter=ACMR "$2...HEAD" -- '*.rs' >"$FILES"
        ;;
    *)
        echo "codescene: expected --all, --staged, --commit [REF], or --base REF" >&2
        exit 2
        ;;
esac

status=0
checked=0
while IFS= read -r file; do
    [ -n "$file" ] || continue
    [ -f "$file" ] || continue
    checked=$((checked + 1))
    if ! output="$("$CS" review --output-format json "$file")"; then
        echo "codescene: review failed for $file" >&2
        status=1
        continue
    fi
    if ! score="$(printf '%s' "$output" | python3 -c '
import json
import sys
data = json.load(sys.stdin)
score = data.get("score")
print("ineligible" if score is None else score)
')"; then
        echo "codescene: invalid JSON for $file. Output was:" >&2
        printf '%s\n' "$output" >&2
        status=1
        continue
    fi
    if [ "$score" = "ineligible" ]; then
        echo "codescene: $file has no scorable code (ineligible)"
    elif [ "$(printf '%s' "$score" | awk '{print ($1 < 10) ? 1 : 0}')" = 1 ]; then
        echo "codescene: $file scored $score (< 10)" >&2
        status=1
    else
        echo "codescene: $file scored $score"
    fi
done <"$FILES"

if [ "$checked" -eq 0 ]; then
    echo "codescene: no changed eligible files"
fi
exit "$status"
