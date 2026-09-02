use super::*;

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
