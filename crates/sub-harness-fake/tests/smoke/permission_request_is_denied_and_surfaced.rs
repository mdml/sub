use super::*;

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
