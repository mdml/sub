use super::*;

#[tokio::test(flavor = "current_thread")]
async fn launch_and_stream_consumption_minimal() {
    let harness = ContractHarness::select(FakeScenario::ReplayMinimal);
    let (_handle, result) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(10)),
            ..PromptOptions::default()
        },
    )
    .await
    .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
    assert!(
        result
            .updates
            .iter()
            .any(|update| update.kind == StreamUpdateKind::AgentMessageChunk),
        "expected message chunks in the stream"
    );
    assert!(result.final_text.contains("Hello"));
    assert_eq!(result.usage, None, "minimal fixture reports no turn usage");
}
