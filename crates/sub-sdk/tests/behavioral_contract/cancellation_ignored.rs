use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_ignored() {
    let harness = ContractHarness::select(FakeScenario::IgnoreCancel);
    let error = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_millis(500)),
            cancel_after: Some(Duration::from_millis(50)),
            ..PromptOptions::default()
        },
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("ignored cancellation should reach the client timeout"));

    assert!(matches!(error, AcpError::TimedOut(_)));
}
