use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_honored() {
    let harness = ContractHarness::select(FakeScenario::CancelHonored);
    let (_handle, result) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            cancel_after: Some(Duration::from_millis(50)),
            ..PromptOptions::default()
        },
    )
    .await
    .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::Cancelled);
}
