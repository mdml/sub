use super::*;

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
                cancel_after: Some(Duration::from_millis(10)),
                ..PromptOptions::default()
            },
        )
        .await
    else {
        panic!("hang scenario should time out");
    };

    assert!(matches!(error, AcpError::TimedOut(_)));
}
