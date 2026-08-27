# Task runner entry points. `just` is pinned in mise.toml.

# Per-commit gate: format, lint, build, docs, tests with coverage.
verify:
    scripts/verify.sh

# Full gate: per-commit gate plus dependency audit and CodeScene.
verify-full:
    scripts/verify.sh --full

# Apply formatting (the gates only check it).
fmt:
    cargo fmt --all

# Dependency-freshness and vulnerability checks, as the GitHub nightly runs them.
nightly-deps:
    cargo deny --locked check advisories
    cargo update --dry-run
