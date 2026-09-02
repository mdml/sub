use super::*;

#[tokio::test(flavor = "current_thread")]
async fn recorded_cursor_fixture_has_activity_without_usage() {
    let harness = ContractHarness::select(FakeScenario::ReplayCursor);
    let (_handle, result) = prompt(
        &harness,
        PromptOptions {
            timeout: Some(Duration::from_secs(30)),
            ..PromptOptions::default()
        },
    )
    .await
    .unwrap_or_else(|error| panic!("prompt turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
    assert!(result.updates.len() > 10);
    assert!(
        result
            .updates
            .iter()
            .any(|update| update.kind == StreamUpdateKind::ToolCall)
    );
    assert_eq!(result.usage, None);
    assert!(result.updates.iter().all(|update| update.cost.is_none()));
}
