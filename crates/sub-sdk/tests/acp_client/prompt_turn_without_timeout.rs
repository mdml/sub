use super::*;

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
