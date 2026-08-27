# Toolchain and tool pinning

Date: 2026-08-27. Status: adopted.

## Decision

The Rust toolchain is pinned by `rust-toolchain.toml` (`1.97.1` with `rustfmt`, `clippy`, `llvm-tools`), and the workspace declares `rust-version = "1.97"`. Developer tools that are not part of the toolchain are pinned in `mise.toml`: `just` 1.58.0, `cargo-llvm-cov` 0.8.7, `cargo-deny` 0.20.2. CI installs the same versions explicitly. `Cargo.lock` is committed and every gate runs with `--locked`.

## Rationale

The verification entry point must produce the same answer for the three harnesses, CI, and the owner, and that is only true if every one of them runs the same compiler, linter, and coverage tool: clippy lints and coverage numbers both shift between versions. `rustup` is the standard way to pin the compiler, and `mise` is already how the owner provisions tools, installs into its own directories without system packages, and can pin `cargo:` crates. The alternatives, a floating `stable` channel or unpinned `cargo install`, would make gate results drift between machines.

## Revisit when

A toolchain upgrade is wanted (bump all three files in one PR and re-run the full gate).
