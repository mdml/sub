use super::*;

/// Run the full contract suite against the harness selected by environment.
///
/// Used by `scripts/nightly/harness-compatibility.sh` in real-harness mode.
#[tokio::test(flavor = "current_thread")]
async fn real_harness_mode_entrypoint() {
    if !real_harness_enabled() {
        eprintln!("SUB_CONTRACT_REAL_HARNESS unset; real-harness entrypoint skipped");
        return;
    }

    let harness = ContractHarness::select(FakeScenario::ReplayMinimal);
    let cwd = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let (handle, result) = client(harness.launch())
        .prompt_turn(
            cwd.path(),
            PROMPT,
            PromptOptions {
                timeout: Some(Duration::from_mins(2)),
                ..PromptOptions::default()
            },
        )
        .await
        .unwrap_or_else(|error| panic!("real harness prompt turn: {error}"));
    let (resumed, resumed_result) = client(harness.launch())
        .prompt_turn(
            cwd.path(),
            "Continue the contract probe and reply briefly.",
            PromptOptions {
                timeout: Some(Duration::from_mins(2)),
                session_start: if harness.real_name() == Some("cursor-agent") {
                    SessionStart::Load(handle.session_id.clone())
                } else {
                    SessionStart::Resume(handle.session_id.clone())
                },
                ..PromptOptions::default()
            },
        )
        .await
        .unwrap_or_else(|error| panic!("real harness resume turn: {error}"));
    let (_, cancelled_result) = client(harness.launch())
        .prompt_turn(
            cwd.path(),
            "Run `sleep 30`, then reply with done.",
            PromptOptions {
                timeout: Some(Duration::from_secs(15)),
                cancel_after: Some(Duration::from_millis(500)),
                ..PromptOptions::default()
            },
        )
        .await
        .unwrap_or_else(|error| panic!("real harness cancel turn: {error}"));

    assert_eq!(result.stop_reason, StopReason::EndTurn);
    assert_eq!(resumed.session_id, handle.session_id);
    assert_eq!(resumed_result.stop_reason, StopReason::EndTurn);
    assert_eq!(cancelled_result.stop_reason, StopReason::Cancelled);
    assert_usage_support(&harness, &result);
}

fn assert_usage_support(harness: &ContractHarness, result: &sub_sdk::acp::PromptResult) {
    match harness.real_name() {
        Some("claude") => {
            assert!(
                result.usage.is_some(),
                "claude should report per-turn tokens"
            );
            assert!(
                result.updates.iter().any(|update| update.cost.is_some()),
                "claude should stream cumulative cost"
            );
        }
        Some("codex") => {
            assert!(
                result.usage.is_some(),
                "codex should report per-turn tokens"
            );
            assert!(
                result.updates.iter().all(|update| update.cost.is_none()),
                "codex should not report cost"
            );
        }
        Some("cursor-agent") => {
            assert_eq!(result.usage, None, "cursor should not report token usage");
            assert!(
                result.updates.iter().all(|update| update.cost.is_none()),
                "cursor should not report cost"
            );
        }
        Some(other) => panic!("real harness outside Observe scope: {other}"),
        None => unreachable!("real-harness entrypoint selected a fake"),
    }
}
