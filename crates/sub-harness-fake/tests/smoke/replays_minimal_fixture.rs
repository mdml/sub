use super::*;

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
