use super::*;

#[tokio::test(flavor = "current_thread")]
async fn load_replay_usage_is_not_counted_twice() {
    if real_harness_enabled() {
        return;
    }
    let harness = ContractHarness::select(FakeScenario::ReplayUsage);
    let cwd = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let (handle, initial_result) = client(harness.launch())
        .prompt_turn(cwd.path(), PROMPT, PromptOptions::default())
        .await
        .unwrap_or_else(|error| panic!("initial turn: {error}"));
    let (_, result) = client(harness.launch())
        .prompt_turn(
            cwd.path(),
            "continue",
            PromptOptions {
                session_start: SessionStart::Load(handle.session_id),
                ..PromptOptions::default()
            },
        )
        .await
        .unwrap_or_else(|error| panic!("loaded turn: {error}"));
    let cost_updates = result
        .updates
        .iter()
        .filter(|update| update.cost.is_some())
        .count();
    let initial_cost_updates = initial_result
        .updates
        .iter()
        .filter(|update| update.cost.is_some())
        .count();
    assert_eq!(cost_updates, initial_cost_updates);
    assert_eq!(result.usage, initial_result.usage);
}
