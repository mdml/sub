# Release tooling: cargo-dist

Date: 2026-08-27. Status: adopted, with channel triggering and prerelease publication superseded by [`2026-09-02-release-channels.md`](2026-09-02-release-channels.md).

## Decision

`cargo-dist` (`dist`) 0.32.0 builds release archives for macOS (arm64, x86_64) and Linux (x86_64, arm64), produces `sha256` checksums, publishes a GitHub Release, and generates the Homebrew formula and `cargo binstall` metadata. Artifacts are signed with `cosign` keyless (Sigstore, GitHub OIDC) through cargo-dist's attestation support. `mise` installs from the GitHub Release through its `ubi` backend, which needs no extra publishing. Configuration is in `dist-workspace.toml`; the generated `release.yml` workflow runs only on `v*` tags, which no one has pushed. Windows is out of scope.

## Rationale

The mental model's distribution decision is `brew` and `mise` plus `cargo binstall`, with signed releases and checksums. `cargo-dist` is the one tool that produces all three from a single config: it writes the Homebrew formula into a tap, emits the `binstall`-compatible archive naming and checksums, and creates the GitHub Release that `mise`'s `ubi` backend installs from. Keyless signing avoids holding a signing key, which matches `sub` never holding credentials. The alternatives, hand-written workflows or `goreleaser`, would need three integrations to be maintained separately.

## Revisit when

A first release is cut (the Homebrew tap repository must exist first; see `docs/release.md`), or `cargo-dist` changes its signing story.
