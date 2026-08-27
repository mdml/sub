# Coverage tool: cargo-llvm-cov

Date: 2026-08-27. Status: adopted.

## Decision

`cargo-llvm-cov` 0.8.7, run with `--workspace --fail-under-lines 90`. The threshold is line coverage over every crate, including binaries, which are exercised by integration tests that run the built executable.

## Rationale

The mental model requires coverage to be measured, not asserted, and above 90 % on `main`. `cargo-llvm-cov` uses the compiler's own source-based instrumentation (`-C instrument-coverage`), so it is exact, works on stable with the `llvm-tools` component already pinned in `rust-toolchain.toml`, instruments binaries run from integration tests, and has a built-in failure threshold, which is what lets a shell script enforce the gate with no extra parsing. The alternative, `tarpaulin`, is ptrace-based, Linux-only, and less exact.

## Revisit when

Branch coverage on stable becomes reliable enough to gate on (then add `--fail-under-branches`).
