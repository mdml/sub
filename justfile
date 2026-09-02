# Task runner entry points. `just` is pinned in mise.toml.

# Fast per-commit gate: compiler/test/coverage checks plus staged-file CodeScene.
verify:
    scripts/verify.sh

# Full PR gate: compiler/test/coverage checks, dependency audit, and base-relative CodeScene.
verify-full:
    scripts/verify.sh --full

# Apply formatting (the gates only check it).
fmt:
    cargo fmt --all

# Dependency-freshness and vulnerability checks, as the GitHub nightly runs them.
nightly-deps:
    cargo deny --locked check advisories
    cargo update --dry-run
