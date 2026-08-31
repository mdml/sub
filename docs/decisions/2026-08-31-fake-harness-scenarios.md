# Fake harness scenario scripting

Date: 2026-08-31. Status: adopted.

## Decision

Scenarios live in `crates/sub-harness-fake/scenarios/` as `{name}.scenario.toml`. Each file names a `fixture` directory and a `behavior` tag: `replay` (default replay), `hang` (never completes the prompt), `die_mid_stream` with `after_events`, `ignore_cancel` (replay through `session/cancel`), `cancel_honored` (replay fixture whose prompt ends with `stop_reason = "cancelled"`, or future live-cancel handling), `malformed` with `after_events` (emit invalid JSON on stdout), or `permission_request` (ask the client to authorize a synthetic tool call and complete only after the response). The `sub-harness-fake` binary selects a scenario by first CLI argument (or `SUB_FAKE_SCENARIO`); fixture and scenario roots default to the crate's bundled directories and may be overridden with `SUB_FAKE_FIXTURES_DIR` and `SUB_FAKE_SCENARIOS_DIR`. The scenario types and replay server belong to `sub-harness-fake`, not the `sub-sdk` kernel.

## Rationale

Separating fixtures (data) from scenarios (behavior) lets one recorded stream exercise multiple failure modes without duplicating JSONL. CLI args are more reliable than environment alone for passing the scenario name to a spawned child. The behavioral contract suite in `sub-sdk` drives the same scenarios through the shared ACP client layer; opt-in real-harness mode reuses that suite when `SUB_CONTRACT_REAL_HARNESS` is set.

## Revisit when

Live `session/cancel` honored mid-replay is required in the fake harness (today `cancel_honored` uses a synthetic fixture stop reason; `ignore_cancel` exercises cancel being ignored during replay).
