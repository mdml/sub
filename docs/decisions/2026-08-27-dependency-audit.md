# Dependency audit: cargo-deny

Date: 2026-08-27. Status: adopted.

## Decision

`cargo-deny` 0.20.2 with `deny.toml` checks advisories (RustSec), licenses (an explicit allow-list compatible with Apache-2.0), duplicate versions (warn), and crate sources (crates.io only). It runs in the full gate and as the GitHub nightly's vulnerability check. Dependabot proposes Cargo and GitHub Actions updates weekly into `staging`.

## Rationale

The mental model asks for vulnerability and dependency-freshness checks on GitHub nightly; `cargo-deny` covers vulnerabilities (the same database as `cargo-audit`) and adds the license and source checks that a public Apache-2.0 project with a simplicity promise needs, in one tool with one config file. Freshness is split: the nightly reports what `cargo update --dry-run` would change, and Dependabot turns those into reviewable PRs, so exact pins stay exact until a human merges a bump.

## Revisit when

A dependency needs a license outside the allow-list (add it here with a reason), or Dependabot noise outweighs its value.
