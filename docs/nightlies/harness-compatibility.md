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

The contract suite lives in `crates/sub-sdk/tests/behavioral_contract.rs`. CI runs the fake-harness cases on every per-commit gate. Install a bridge explicitly, then set `SUB_CONTRACT_REAL_HARNESS` and `SUB_CONTRACT_HARNESS_CMD` to the printed bridge path before running:

```sh
SUB_CONTRACT_REAL_HARNESS=codex SUB_CONTRACT_HARNESS_CMD=/path/printed/by/sub cargo test -p sub-sdk --test behavioral_contract real_harness_mode_entrypoint
```

The nightly reads `SUB_CONTRACT_CLAUDE_CMD` and `SUB_CONTRACT_CODEX_CMD` for previously installed bridge paths, sets the user's harness executable side channels, and runs `real_harness_mode_entrypoint`. It never installs or fetches a bridge. Cursor continues to use its native `acp` command.

## Status

The contract suite, fake harness, Claude adapter, and Codex adapter exist. The nightly reports their pinned and verified versions; automatic comparison and the Cursor adapter remain later work.
