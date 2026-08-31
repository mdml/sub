//! Unit tests for the shared ACP client layer.

#[path = "common/fake_binary.rs"]
mod fake_binary;

use std::time::Duration;

use fake_binary::fake_binary;

use sub_harness_fake::{FixtureSource, LoadedFixture, Scenario, ScenarioBehavior};
use sub_sdk::acp::{
    AcpClient, AcpClientConfig, AcpError, HarnessLaunch, PromptOptions, StopReason,
};

#[test]
fn harness_launch_builder() {
    let launch = HarnessLaunch::new("sub-harness-fake")
        .arg("replay-minimal")
        .env("SUB_FAKE_FIXTURES_DIR", "/tmp/fixtures");
    assert_eq!(launch.command(), std::path::Path::new("sub-harness-fake"));
    assert_eq!(launch.args(), &["replay-minimal".to_owned()]);
    assert_eq!(
        launch.environment().get("SUB_FAKE_FIXTURES_DIR"),
        Some(&"/tmp/fixtures".to_owned())
    );
}

#[test]
fn acp_client_config_defaults() {
    let config = AcpClientConfig::default();
    assert_eq!(config.client_name, "sub");
}

#[test]
fn minimal_fixture_loads() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sub-harness-fake/fixtures/minimal");
    let fixture = LoadedFixture::load(dir).unwrap_or_else(|error| panic!("fixture: {error}"));
    assert!(matches!(fixture.manifest.source, FixtureSource::Synthetic));
    assert_eq!(fixture.manifest.prompt.stop_reason, StopReason::EndTurn);
}

#[test]
fn codex_fixture_is_recorded() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sub-harness-fake/fixtures/codex-hello");
    let fixture = LoadedFixture::load(dir).unwrap_or_else(|error| panic!("fixture: {error}"));
    assert!(matches!(
        fixture.manifest.source,
        FixtureSource::Recorded { .. }
    ));
    assert!(fixture.events.len() > 100);
}

#[test]
fn hang_scenario_deserializes() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sub-harness-fake/scenarios/hang.scenario.toml");
    let scenario = Scenario::load(path).unwrap_or_else(|error| panic!("scenario: {error}"));
    assert_eq!(scenario.behavior, ScenarioBehavior::Hang);
}

#[tokio::test(flavor = "current_thread")]
async fn fake_client_prompt_turn_minimal() {
    let fixtures =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/fixtures");
    let scenarios =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/scenarios");

    let launch = HarnessLaunch::new(fake_binary())
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
            "probe",
            PromptOptions {
                timeout: Some(Duration::from_secs(10)),
                ..PromptOptions::default()
            },
        )
        .await
        .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
}

#[tokio::test(flavor = "current_thread")]
async fn prompt_turn_times_out_on_hang() {
    let fixtures =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/fixtures");
    let scenarios =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/scenarios");

    let launch = HarnessLaunch::new(fake_binary())
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
            "timeout probe",
            PromptOptions {
                timeout: Some(Duration::from_millis(200)),
                ..PromptOptions::default()
            },
        )
        .await
    else {
        panic!("hang scenario should time out");
    };

    assert!(matches!(error, AcpError::TimedOut(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn missing_agent_binary_fails() {
    let launch = HarnessLaunch::new("/nonexistent/sub-harness-fake");
    let Err(error) = AcpClient::new(launch, AcpClientConfig::default())
        .prompt_turn(
            std::env::temp_dir(),
            "missing",
            PromptOptions {
                timeout: Some(Duration::from_secs(1)),
                ..PromptOptions::default()
            },
        )
        .await
    else {
        panic!("missing binary should fail");
    };

    assert!(matches!(
        error,
        AcpError::Protocol(_) | AcpError::Io(_) | AcpError::ProcessExited
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn prompt_turn_without_timeout() {
    let fixtures =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/fixtures");
    let scenarios =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/scenarios");

    let launch = HarnessLaunch::new(fake_binary())
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
        .prompt_turn(std::env::temp_dir(), "no timeout", PromptOptions::default())
        .await
        .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
}
