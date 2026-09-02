use super::*;

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
