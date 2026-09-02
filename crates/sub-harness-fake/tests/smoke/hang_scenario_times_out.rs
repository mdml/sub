use super::*;

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
