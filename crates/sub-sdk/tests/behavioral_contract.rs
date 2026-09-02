//! Behavioral contract suite for ACP harnesses.
//!
//! By default these tests drive the programmable fake harness. Set
//! `SUB_CONTRACT_REAL_HARNESS` to `claude`, `codex`, or `cursor-agent` to run
//! the same assertions against a local harness. Claude and Codex also require
//! `SUB_CONTRACT_HARNESS_CMD` to name a bridge installed by `sub`.

mod common;

use std::time::Duration;

use common::harness::{ContractHarness, FakeScenario, real_harness_enabled};
use sub_sdk::acp::{
    AcpClient, AcpClientConfig, AcpError, HarnessLaunch, PromptOptions, SessionStart, StopReason,
    StreamUpdateKind,
};
use tempfile::TempDir;

const PROMPT: &str = "Reply with exactly: contract suite probe complete";

fn client(launch: HarnessLaunch) -> AcpClient {
    AcpClient::new(launch, AcpClientConfig::default())
}

async fn prompt(
    harness: &ContractHarness,
    options: PromptOptions,
) -> Result<(sub_sdk::acp::SessionHandle, sub_sdk::acp::PromptResult), AcpError> {
    let cwd = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
    client(harness.launch())
        .prompt_turn(cwd.path(), PROMPT, options)
        .await
}
#[path = "behavioral_contract/cancellation_honored.rs"]
mod cancellation_honored;
#[path = "behavioral_contract/cancellation_ignored.rs"]
mod cancellation_ignored;
#[path = "behavioral_contract/cross_process_resume_continues_the_same_session.rs"]
mod cross_process_resume_continues_the_same_session;
#[path = "behavioral_contract/death_mid_stream.rs"]
mod death_mid_stream;
#[path = "behavioral_contract/hang_times_out.rs"]
mod hang_times_out;
#[path = "behavioral_contract/launch_and_stream_consumption_minimal.rs"]
mod launch_and_stream_consumption_minimal;
#[path = "behavioral_contract/launch_and_stream_consumption_recorded_codex_fixture.rs"]
mod launch_and_stream_consumption_recorded_codex_fixture;
#[path = "behavioral_contract/load_replay_updates_are_not_counted_as_continuation_updates.rs"]
mod load_replay_updates_are_not_counted_as_continuation_updates;
#[path = "behavioral_contract/load_replay_usage_is_not_counted_twice.rs"]
mod load_replay_usage_is_not_counted_twice;
#[path = "behavioral_contract/malformed_output.rs"]
mod malformed_output;
#[path = "behavioral_contract/permission_request_is_denied_and_surfaced.rs"]
mod permission_request_is_denied_and_surfaced;
#[path = "behavioral_contract/real_harness_mode_entrypoint.rs"]
mod real_harness_mode_entrypoint;
#[path = "behavioral_contract/recorded_cursor_fixture_has_activity_without_usage.rs"]
mod recorded_cursor_fixture_has_activity_without_usage;
#[path = "behavioral_contract/resume_refused_by_harness_is_an_error.rs"]
mod resume_refused_by_harness_is_an_error;
