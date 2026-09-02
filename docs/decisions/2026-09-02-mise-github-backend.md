# Mise GitHub Release backend

Date: 2026-09-02. Status: adopted; supersedes the mise-backend portion of [`2026-08-27-release-tooling.md`](2026-08-27-release-tooling.md).

## Decision

Install `sub` from GitHub Releases with `mise use github:mdml/sub`. Release publication remains unchanged: cargo-dist publishes stable archives, and the dedicated nightly workflow publishes prerelease archives.

## Rationale

Mise has deprecated its `ubi` backend and directs GitHub Release users to replace `ubi:owner/repo` with [`github:owner/repo`](https://mise.jdx.dev/dev-tools/backends/ubi.html). The GitHub backend directly installs release assets, adds provenance verification and download progress, and preserves the repository's no-extra-publishing-step distribution model.

## Revisit when

Revisit if `sub` receives a mise registry entry or its release archive layout no longer supports mise's GitHub asset autodetection.
