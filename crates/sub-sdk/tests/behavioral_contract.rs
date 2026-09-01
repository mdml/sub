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
    AcpClient, AcpClientConfig, AcpError, HarnessLaunch, PromptOptions, StopReason,
    StreamUpdateKind,
};
use tempfile::TempDir;

const PROMPT: &str = "contract suite probe";

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

#[tokio::test(flavor = "current_thread")]
async fn launch_and_stream_consumption_minimal() {
    let harness = ContractHarness::select(FakeScenario::ReplayMinimal);
    let (_handle, result) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            ..PromptOptions::default()
        },
    )
    .await
    .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
    assert!(
        result
            .updates
            .iter()
            .any(|update| update.kind == StreamUpdateKind::AgentMessageChunk),
        "expected message chunks in the stream"
    );
    assert!(result.final_text.contains("Hello"));
    assert_eq!(result.usage, None, "minimal fixture reports no turn usage");
}

#[tokio::test(flavor = "current_thread")]
async fn launch_and_stream_consumption_recorded_codex_fixture() {
    let harness = ContractHarness::select(FakeScenario::ReplayCodex);
    let (_handle, result) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(30)),
            ..PromptOptions::default()
        },
    )
    .await
    .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
    assert!(
        result.updates.len() > 10,
        "recorded codex fixture should replay many updates, got {}",
        result.updates.len()
    );
    let usage = result
        .usage
        .unwrap_or_else(|| panic!("recorded codex fixture should report per-turn usage"));
    assert_eq!(usage.total_tokens, 16_749);
    assert_eq!(usage.input_tokens, 1_410);
    assert_eq!(usage.output_tokens, 235);
}

#[tokio::test(flavor = "current_thread")]
async fn permission_request_is_denied_and_surfaced() {
    let harness = ContractHarness::select(FakeScenario::PermissionRequest);
    let (_handle, result) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            ..PromptOptions::default()
        },
    )
    .await
    .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
    let denial = result
        .updates
        .iter()
        .find(|update| update.kind == StreamUpdateKind::PermissionDenied)
        .unwrap_or_else(|| panic!("permission denial update"));
    assert_eq!(denial.text.as_deref(), Some("Write fixture output"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_honored() {
    let harness = ContractHarness::select(FakeScenario::CancelHonored);
    let (_handle, result) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            ..PromptOptions::default()
        },
    )
    .await
    .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::Cancelled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_ignored() {
    let harness = ContractHarness::select(FakeScenario::IgnoreCancel);
    let (_handle, result) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            cancel_after: Some(Duration::from_millis(50)),
            ..PromptOptions::default()
        },
    )
    .await
    .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
}

#[tokio::test(flavor = "current_thread")]
async fn death_mid_stream() {
    let harness = ContractHarness::select(FakeScenario::DieMidStream);
    let Err(error) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            ..PromptOptions::default()
        },
    )
    .await
    else {
        panic!("agent death should fail the turn");
    };

    assert!(
        matches!(
            error,
            AcpError::Protocol(_)
                | AcpError::StreamEnded
                | AcpError::ProcessExited
                | AcpError::Io(_)
        ),
        "unexpected error: {error:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn hang_times_out() {
    let harness = ContractHarness::select(FakeScenario::Hang);
    let Err(error) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_millis(500)),
            ..PromptOptions::default()
        },
    )
    .await
    else {
        panic!("hang without cancel should time out");
    };

    assert!(matches!(error, AcpError::TimedOut(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_output() {
    let harness = ContractHarness::select(FakeScenario::Malformed);
    let Err(error) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            ..PromptOptions::default()
        },
    )
    .await
    else {
        panic!("malformed output should fail the turn");
    };

    assert!(
        matches!(
            error,
            AcpError::Protocol(_) | AcpError::StreamEnded | AcpError::Io(_)
        ),
        "unexpected error: {error:?}"
    );
}

/// Run the full contract suite against the harness selected by environment.
///
/// Used by `scripts/nightly/harness-compatibility.sh` in real-harness mode.
#[tokio::test(flavor = "current_thread")]
async fn real_harness_mode_entrypoint() {
    if !real_harness_enabled() {
        eprintln!("SUB_CONTRACT_REAL_HARNESS unset; real-harness entrypoint skipped");
        return;
    }

    let harness = ContractHarness::select(FakeScenario::ReplayMinimal);
    let (_handle, result) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_mins(2)),
            ..PromptOptions::default()
        },
    )
    .await
    .unwrap_or_else(|error| panic!("real harness prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
    match harness.real_name() {
        Some("claude") => {
            assert!(
                result.usage.is_some(),
                "claude should report per-turn tokens"
            );
            assert!(
                result.updates.iter().any(|update| update.cost.is_some()),
                "claude should stream cumulative cost"
            );
        }
        Some("codex") => {
            assert!(
                result.usage.is_some(),
                "codex should report per-turn tokens"
            );
            assert!(
                result.updates.iter().all(|update| update.cost.is_none()),
                "codex should not report cost"
            );
        }
        Some(other) => panic!("real harness outside Observe scope: {other}"),
        None => unreachable!("real-harness entrypoint selected a fake"),
    }
}
