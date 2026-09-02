use super::*;

#[tokio::test(flavor = "current_thread")]
async fn hang_times_out() {
    let harness = ContractHarness::select(FakeScenario::Hang);
    let Err(error) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_millis(500)),
            ..PromptOptions::default()
        },
    )
    .await
    else {
        panic!("hang without cancel should time out");
    };

    assert!(matches!(error, AcpError::TimedOut(_)));
}
