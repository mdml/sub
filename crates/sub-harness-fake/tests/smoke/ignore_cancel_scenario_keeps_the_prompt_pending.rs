use super::*;

#[tokio::test(flavor = "current_thread")]
async fn ignore_cancel_scenario_keeps_the_prompt_pending() {
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

    let error = AcpClient::new(launch, AcpClientConfig::default())
        .prompt_turn(
            std::env::temp_dir(),
            "ignore cancel",
            PromptOptions {
                timeout: Some(Duration::from_millis(500)),
                cancel_after: Some(Duration::from_millis(50)),
                ..PromptOptions::default()
            },
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("ignored cancellation must keep the prompt pending"));

    assert!(matches!(error, AcpError::TimedOut(_)));
}
