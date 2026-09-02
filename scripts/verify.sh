#!/usr/bin/env sh
# The single verification entry point for this repository.
#
#   scripts/verify.sh                  fast gate, including staged-file CodeScene
#   scripts/verify.sh --full           PR gate, including base-relative CodeScene and cargo-deny
#   scripts/verify.sh --full --base R  PR gate relative to base ref R
#   scripts/verify.sh --full --all     full gate with whole-tree CodeScene
set -eu

COVERAGE_MIN="${SUB_COVERAGE_MIN:-90}"
BASE_REF="${SUB_VERIFY_BASE:-origin/staging}"
FULL=0
ALL=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --full) FULL=1 ;;
        --all) ALL=1 ;;
        --base)
            [ "$#" -ge 2 ] || { echo "verify: --base requires a ref" >&2; exit 2; }
            BASE_REF="$2"
            shift
            ;;
        -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
        *) echo "verify: unknown argument '$1'" >&2; exit 2 ;;
    esac
    shift
done
[ "$ALL" -eq 0 ] || [ "$FULL" -eq 1 ] || {
    echo "verify: --all requires --full" >&2
    exit 2
}

cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }
need() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "verify: '$1' is required but not installed. Run 'mise install' (see README.md)." >&2
        exit 2
    fi
}

need cargo
need cargo-llvm-cov

step "format (rustfmt --check)"
cargo fmt --all -- --check

step "lint (clippy, warnings are errors)"
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

step "typecheck and build"
cargo build --workspace --all-targets --all-features --locked

step "docs (rustdoc, warnings are errors)"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

step "tests with coverage (cargo-llvm-cov, fail under ${COVERAGE_MIN}%)"
cargo llvm-cov --workspace --all-features --locked \
    --fail-under-lines "$COVERAGE_MIN" \
    --summary-only

if [ "$FULL" -eq 1 ]; then
    step "dependency audit (cargo-deny: advisories, licenses, bans, sources)"
    need cargo-deny
    cargo deny --locked check

    if [ "$ALL" -eq 1 ]; then
        step "CodeScene (whole tracked tree, code health 10)"
        scripts/codescene.sh --all
    else
        step "CodeScene (files changed from $BASE_REF, code health 10)"
        scripts/codescene.sh --base "$BASE_REF"
    fi
elif [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    step "CodeScene (files changed by HEAD, code health 10)"
    scripts/codescene.sh --commit HEAD
else
    step "CodeScene (staged files, code health 10)"
    scripts/codescene.sh --staged
fi

step "verify: OK"
