# Nightly: harness compatibility

Purpose: detect that a real harness or bridge has drifted from what the adapters were verified against, before a user does.

## Job definition

1. Record the installed versions of `claude`, `codex`, `cursor-agent`, and each pinned bridge.
2. Compare them with the versions each adapter declares. A mismatch is reported, not failed: the adapters' declared versions are what the fake harness's fixtures were recorded from.
3. Run the behavioral contract suite against each installed real harness (the suite's opt-in real-harness mode), on a throwaway working directory.
4. A test that passes on the fake harness and fails on a real harness is reported as "fake is wrong", per the mental model's verification decision.
5. Write a dated report under a local directory outside the repository and exit non-zero on any failure.

## Scheduling

Run `scripts/nightly/harness-compatibility.sh` once a day from the owner's scheduler with the repository checked out at `staging`. The script requires the harness binaries on `PATH` and does not modify any harness's global configuration.

## Real-harness mode

The contract suite lives in `crates/sub-sdk/tests/behavioral_contract.rs`. CI runs the fake-harness cases on every per-commit gate. Locally, set `SUB_CONTRACT_REAL_HARNESS` to `claude`, `codex`, or `cursor-agent` (optional override: `SUB_CONTRACT_HARNESS_CMD`) before running:

```sh
SUB_CONTRACT_REAL_HARNESS=codex cargo test -p sub-sdk --test behavioral_contract
```

The nightly script sets that variable per installed harness and runs `real_harness_mode_entrypoint`.

## Status

The contract suite and fake harness exist. Adapter version constants and bridge comparison remain stubs until item 3 (adapters); the nightly logs that gap and still runs real-harness contract tests when harnesses are installed.
