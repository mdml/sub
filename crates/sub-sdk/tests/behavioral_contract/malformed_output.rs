use super::*;

#[tokio::test(flavor = "current_thread")]
async fn malformed_output() {
    let harness = ContractHarness::select(FakeScenario::Malformed);
    let Err(error) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            ..PromptOptions::default()
        },
    )
    .await
    else {
        panic!("malformed output should fail the turn");
    };

    assert!(
        matches!(
            error,
            AcpError::Protocol(_) | AcpError::StreamEnded | AcpError::Io(_)
        ),
        "unexpected error: {error:?}"
    );
}
