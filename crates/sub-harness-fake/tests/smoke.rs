//! Smoke test: the instrumented `sub-harness-fake` binary replays a fixture.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use sub_sdk::acp::{
    AcpClient, AcpClientConfig, AcpError, HarnessLaunch, PromptOptions, StopReason,
    StreamUpdateKind,
};

fn harness_binary() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_sub_harness_fake") {
        return std::path::PathBuf::from(path);
    }

    if let Some(path) = option_env!("CARGO_BIN_EXE_sub_harness_fake") {
        return std::path::PathBuf::from(path);
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(path) = harness_binary_near_test_exe(&exe)
    {
        return path;
    }

    panic!("sub-harness-fake binary not found");
}

fn harness_binary_near_test_exe(exe: &Path) -> Option<std::path::PathBuf> {
    let debug_dir = exe.parent()?.parent()?;
    let direct = debug_dir.join("sub-harness-fake");
    if direct.is_file() {
        return Some(direct);
    }

    let sibling = debug_dir
        .parent()?
        .parent()?
        .join("debug")
        .join("sub-harness-fake");
    if sibling.is_file() {
        return Some(sibling);
    }

    None
}
#[path = "smoke/binary_exits_when_scenario_missing.rs"]
mod binary_exits_when_scenario_missing;
#[path = "smoke/binary_reports_unknown_scenario.rs"]
mod binary_reports_unknown_scenario;
#[path = "smoke/die_mid_stream_fails_the_turn.rs"]
mod die_mid_stream_fails_the_turn;
#[path = "smoke/hang_scenario_times_out.rs"]
mod hang_scenario_times_out;
#[path = "smoke/ignore_cancel_scenario_keeps_the_prompt_pending.rs"]
mod ignore_cancel_scenario_keeps_the_prompt_pending;
#[path = "smoke/malformed_output_fails_the_turn.rs"]
mod malformed_output_fails_the_turn;
#[path = "smoke/permission_request_is_denied_and_surfaced.rs"]
mod permission_request_is_denied_and_surfaced;
#[path = "smoke/replays_minimal_fixture.rs"]
mod replays_minimal_fixture;
#[path = "smoke/replays_recorded_codex_fixture.rs"]
mod replays_recorded_codex_fixture;
