use super::*;

#[tokio::test(flavor = "current_thread")]
async fn launch_and_stream_consumption_recorded_codex_fixture() {
    let harness = ContractHarness::select(FakeScenario::ReplayCodex);
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
    assert!(
        result.updates.len() > 10,
        "recorded codex fixture should replay many updates, got {}",
        result.updates.len()
    );
    let usage = result
        .usage
        .unwrap_or_else(|| panic!("recorded codex fixture should report per-turn usage"));
    assert_eq!(usage.total_tokens, 16_749);
    assert_eq!(usage.input_tokens, 1_410);
    assert_eq!(usage.output_tokens, 235);
}
