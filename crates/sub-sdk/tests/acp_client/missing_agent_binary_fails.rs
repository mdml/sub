use super::*;

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
