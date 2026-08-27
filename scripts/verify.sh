#!/usr/bin/env sh
# The single verification entry point for this repository.
#
#   scripts/verify.sh          per-commit gate (format, lint, build, tests, coverage)
#   scripts/verify.sh --full   full gate: per-commit gate + CodeScene + dependency audit
#
# `just verify` and `just verify-full` call this script; CI calls it directly.
# The gates are defined by the mental model (see AGENTS.md); this script
# implements them. See docs/verification.md for what each step does.
set -eu

COVERAGE_MIN="${SUB_COVERAGE_MIN:-90}"
FULL=0
for arg in "$@"; do
    case "$arg" in
        --full) FULL=1 ;;
        -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
        *) echo "verify: unknown argument '$arg'" >&2; exit 2 ;;
    esac
done

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

    step "CodeScene (code health 10 for eligible files)"
    scripts/codescene.sh
fi

step "verify: OK"
