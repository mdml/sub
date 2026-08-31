//! Smoke test: the instrumented `sub-harness-fake` binary replays a fixture.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use sub_sdk::acp::{
    AcpClient, AcpClientConfig, HarnessLaunch, PromptOptions, StopReason, StreamUpdateKind,
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

#[tokio::test(flavor = "current_thread")]
async fn replays_minimal_fixture() {
    let fixtures = sub_harness_fake::fixtures_dir();
    let scenarios = sub_harness_fake::scenarios_dir();

    let launch = HarnessLaunch::new(harness_binary())
        .arg("replay-minimal")
        .env(
            "SUB_FAKE_FIXTURES_DIR",
            fixtures.to_string_lossy().into_owned(),
        )
        .env(
            "SUB_FAKE_SCENARIOS_DIR",
            scenarios.to_string_lossy().into_owned(),
        );

    let (_handle, result) = AcpClient::new(launch, AcpClientConfig::default())
        .prompt_turn(
            std::env::temp_dir(),
            "smoke",
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
            .any(|update| update.kind == StreamUpdateKind::AgentMessageChunk)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn permission_request_is_denied_and_surfaced() {
    let launch = HarnessLaunch::new(harness_binary())
        .arg("permission-request")
        .env(
            "SUB_FAKE_FIXTURES_DIR",
            sub_harness_fake::fixtures_dir()
                .to_string_lossy()
                .into_owned(),
        )
        .env(
            "SUB_FAKE_SCENARIOS_DIR",
            sub_harness_fake::scenarios_dir()
                .to_string_lossy()
                .into_owned(),
        );

    let (_handle, result) = AcpClient::new(launch, AcpClientConfig::default())
        .prompt_turn(
            std::env::temp_dir(),
            "permission",
            PromptOptions {
                timeout: Some(Duration::from_secs(10)),
                ..PromptOptions::default()
            },
        )
        .await
        .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
    assert!(result.updates.iter().any(|update| {
        update.kind == StreamUpdateKind::PermissionDenied
            && update.text.as_deref() == Some("Write fixture output")
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn ignore_cancel_scenario_completes() {
    let fixtures = sub_harness_fake::fixtures_dir();
    let scenarios = sub_harness_fake::scenarios_dir();

    let launch = HarnessLaunch::new(harness_binary())
        .arg("ignore-cancel")
        .env(
            "SUB_FAKE_FIXTURES_DIR",
            fixtures.to_string_lossy().into_owned(),
        )
        .env(
            "SUB_FAKE_SCENARIOS_DIR",
            scenarios.to_string_lossy().into_owned(),
        );

    let (_handle, result) = AcpClient::new(launch, AcpClientConfig::default())
        .prompt_turn(
            std::env::temp_dir(),
            "ignore cancel",
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
async fn hang_scenario_times_out() {
    let fixtures = sub_harness_fake::fixtures_dir();
    let scenarios = sub_harness_fake::scenarios_dir();

    let launch = HarnessLaunch::new(harness_binary())
        .arg("hang")
        .env(
            "SUB_FAKE_FIXTURES_DIR",
            fixtures.to_string_lossy().into_owned(),
        )
        .env(
            "SUB_FAKE_SCENARIOS_DIR",
            scenarios.to_string_lossy().into_owned(),
        );

    let Err(error) = AcpClient::new(launch, AcpClientConfig::default())
        .prompt_turn(
            std::env::temp_dir(),
            "hang",
            PromptOptions {
                timeout: Some(Duration::from_millis(200)),
                ..PromptOptions::default()
            },
        )
        .await
    else {
        panic!("hang scenario should time out");
    };

    assert!(matches!(error, sub_sdk::acp::AcpError::TimedOut(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn replays_recorded_codex_fixture() {
    let fixtures = sub_harness_fake::fixtures_dir();
    let scenarios = sub_harness_fake::scenarios_dir();

    let launch = HarnessLaunch::new(harness_binary())
        .arg("replay-codex")
        .env(
            "SUB_FAKE_FIXTURES_DIR",
            fixtures.to_string_lossy().into_owned(),
        )
        .env(
            "SUB_FAKE_SCENARIOS_DIR",
            scenarios.to_string_lossy().into_owned(),
        );

    let (_handle, result) = AcpClient::new(launch, AcpClientConfig::default())
        .prompt_turn(
            std::env::temp_dir(),
            "codex replay",
            PromptOptions {
                timeout: Some(Duration::from_secs(30)),
                ..PromptOptions::default()
            },
        )
        .await
        .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
    assert!(result.updates.len() > 10);
}

#[tokio::test(flavor = "current_thread")]
async fn die_mid_stream_fails_the_turn() {
    let fixtures = sub_harness_fake::fixtures_dir();
    let scenarios = sub_harness_fake::scenarios_dir();

    let launch = HarnessLaunch::new(harness_binary())
        .arg("die-mid-stream")
        .env(
            "SUB_FAKE_FIXTURES_DIR",
            fixtures.to_string_lossy().into_owned(),
        )
        .env(
            "SUB_FAKE_SCENARIOS_DIR",
            scenarios.to_string_lossy().into_owned(),
        );

    let Err(error) = AcpClient::new(launch, AcpClientConfig::default())
        .prompt_turn(
            std::env::temp_dir(),
            "die",
            PromptOptions {
                timeout: Some(Duration::from_secs(10)),
                ..PromptOptions::default()
            },
        )
        .await
    else {
        panic!("die mid stream should fail");
    };

    assert!(matches!(
        error,
        sub_sdk::acp::AcpError::Protocol(_)
            | sub_sdk::acp::AcpError::StreamEnded
            | sub_sdk::acp::AcpError::ProcessExited
            | sub_sdk::acp::AcpError::Io(_)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_output_fails_the_turn() {
    let fixtures = sub_harness_fake::fixtures_dir();
    let scenarios = sub_harness_fake::scenarios_dir();

    let launch = HarnessLaunch::new(harness_binary())
        .arg("malformed")
        .env(
            "SUB_FAKE_FIXTURES_DIR",
            fixtures.to_string_lossy().into_owned(),
        )
        .env(
            "SUB_FAKE_SCENARIOS_DIR",
            scenarios.to_string_lossy().into_owned(),
        );

    let Err(error) = AcpClient::new(launch, AcpClientConfig::default())
        .prompt_turn(
            std::env::temp_dir(),
            "malformed",
            PromptOptions {
                timeout: Some(Duration::from_secs(10)),
                ..PromptOptions::default()
            },
        )
        .await
    else {
        panic!("malformed output should fail");
    };

    assert!(matches!(
        error,
        sub_sdk::acp::AcpError::Protocol(_)
            | sub_sdk::acp::AcpError::StreamEnded
            | sub_sdk::acp::AcpError::Io(_)
    ));
}

#[test]
fn binary_exits_when_scenario_missing() {
    let output = Command::new(harness_binary())
        .env_remove("SUB_FAKE_SCENARIO")
        .output()
        .unwrap_or_else(|error| panic!("run binary: {error}"));
    assert!(!output.status.success());
}

#[test]
fn binary_reports_unknown_scenario() {
    let output = Command::new(harness_binary())
        .arg("does-not-exist")
        .output()
        .unwrap_or_else(|error| panic!("run binary: {error}"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("I/O error"));
}
