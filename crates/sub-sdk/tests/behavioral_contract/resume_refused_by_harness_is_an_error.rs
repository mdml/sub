use super::*;

#[tokio::test(flavor = "current_thread")]
async fn resume_refused_by_harness_is_an_error() {
    if real_harness_enabled() {
        return;
    }
    let harness = ContractHarness::select(FakeScenario::ResumeRefused);
    let cwd = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let error = client(harness.launch())
        .prompt_turn(
            cwd.path(),
            "continue",
            PromptOptions {
                session_start: SessionStart::Resume("fixture-session".to_owned()),
                ..PromptOptions::default()
            },
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("resume should fail"));
    assert!(matches!(error, AcpError::Protocol(_)));
}
