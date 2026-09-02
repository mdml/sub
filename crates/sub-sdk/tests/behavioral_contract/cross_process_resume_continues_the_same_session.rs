use super::*;

#[tokio::test(flavor = "current_thread")]
async fn cross_process_resume_continues_the_same_session() {
    let harness = ContractHarness::select(FakeScenario::ReplayMinimal);
    let cwd = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let (first, _) = client(harness.launch())
        .prompt_turn(cwd.path(), PROMPT, PromptOptions::default())
        .await
        .unwrap_or_else(|error| panic!("initial turn: {error}"));
    let (resumed, result) = client(harness.launch())
        .prompt_turn(
            cwd.path(),
            "continue the contract probe",
            PromptOptions {
                timeout: Some(Duration::from_mins(2)),
                session_start: SessionStart::Resume(first.session_id.clone()),
                ..PromptOptions::default()
            },
        )
        .await
        .unwrap_or_else(|error| panic!("resumed turn: {error}"));

    assert_eq!(resumed.session_id, first.session_id);
    assert_eq!(result.stop_reason, StopReason::EndTurn);
}
