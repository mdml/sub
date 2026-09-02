use super::*;

#[tokio::test(flavor = "current_thread")]
async fn load_replay_updates_are_not_counted_as_continuation_updates() {
    if real_harness_enabled() {
        return;
    }
    let harness = ContractHarness::select(FakeScenario::ReplayMinimal);
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
    let message_chunks = result
        .updates
        .iter()
        .filter(|update| update.kind == StreamUpdateKind::AgentMessageChunk)
        .count();
    let initial_message_chunks = initial_result
        .updates
        .iter()
        .filter(|update| update.kind == StreamUpdateKind::AgentMessageChunk)
        .count();
    assert_eq!(
        message_chunks, initial_message_chunks,
        "only the continuation stream is observed"
    );
}
