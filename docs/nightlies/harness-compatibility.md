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

## Status

Definition only. The contract suite and adapters do not exist yet; the script exits with a message saying so.
