# Async runtime: tokio

Date: 2026-08-27. Status: adopted.

## Decision

`tokio`, pinned exactly to `1.53.1` (the latest stable on crates.io on 2026-08-27), with the `rt-multi-thread`, `macros`, `process`, `io-util`, `sync`, and `time` features. Declared once in the workspace and used by every crate.

## Rationale

`sub`'s runtime work is spawning bridge and harness processes, multiplexing their stdio as JSON-RPC, timing out waits, and cancelling attempts; tokio's `process`, `io-util`, `sync`, and `time` modules cover all of that without further crates. The `agent-client-protocol` crate's own examples and its `tokio` feature target tokio, so no runtime adaptor is needed at the ACP boundary. The pin is exact because the mental model's promise is code that is simple to audit: an exact version plus `Cargo.lock` means the audited dependency is the built dependency, and freshness is handled by the nightly and by Dependabot proposals rather than by floating ranges. The alternatives, `smol` or `async-std`, would need an adaptor at the ACP boundary and have a smaller process-management surface.

## Revisit when

The ACP SDK drops tokio support, or a single-threaded runtime proves sufficient for the supervisor process and the multi-thread runtime measurably costs startup time.
