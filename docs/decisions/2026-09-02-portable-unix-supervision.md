# Portable Unix supervisor detachment and identity

Date: 2026-09-02. Status: adopted.

## Decision

On every supported Unix target, `sub` starts each supervisor directly and calls `setsid(2)` in the child's `pre_exec` hook. The spawn no longer depends on an external `setsid` executable. The `libc` crate is an exact direct dependency and supplies the platform bindings; the workspace denies unsafe code by default and permits each audited process-control FFI site locally.

Running attempts retain a supervisor PID and operating-system start token. Linux reads the process state and start ticks from `/proc/<pid>/stat`. macOS calls `proc_pidinfo` with `PROC_PIDTBSDINFO`, rejects zombies, and represents `pbi_start_tvsec` plus `pbi_start_tvusec` as microseconds since the Unix epoch. Observation reports a supervisor as live only when both PID and start token match. The former PID-only `kill -0` fallback is removed. The release targets remain Linux and macOS; Windows and other Unix targets are not supported.

## Rationale

In-process `setsid(2)` gives Linux and macOS the same detachment mechanism without assuming that a utility binary is installed. PID plus start identity prevents a recycled PID from making an orphaned attempt appear live. Using the exact-pinned, widely audited `libc` bindings avoids maintaining a handwritten `libproc` ABI while keeping the macOS mechanism to one query and one platform structure.

## Revisit when

The release target list adds another operating system, the private persisted process token must survive a representation migration, or a safe standard-library API replaces either FFI call.
