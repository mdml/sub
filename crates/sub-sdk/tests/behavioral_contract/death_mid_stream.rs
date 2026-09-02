use super::*;

#[tokio::test(flavor = "current_thread")]
async fn death_mid_stream() {
    let harness = ContractHarness::select(FakeScenario::DieMidStream);
    let Err(error) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            ..PromptOptions::default()
        },
    )
    .await
    else {
        panic!("agent death should fail the turn");
    };

    assert!(
        matches!(
            error,
            AcpError::Protocol(_)
                | AcpError::StreamEnded
                | AcpError::ProcessExited
                | AcpError::Io(_)
        ),
        "unexpected error: {error:?}"
    );
}
