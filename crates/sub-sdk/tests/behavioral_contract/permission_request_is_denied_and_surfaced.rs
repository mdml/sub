use super::*;

#[tokio::test(flavor = "current_thread")]
async fn permission_request_is_denied_and_surfaced() {
    let harness = ContractHarness::select(FakeScenario::PermissionRequest);
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
    let denial = result
        .updates
        .iter()
        .find(|update| update.kind == StreamUpdateKind::PermissionDenied)
        .unwrap_or_else(|| panic!("permission denial update"));
    assert_eq!(denial.text.as_deref(), Some("Write fixture output"));
    let denied = result
        .updates
        .iter()
        .filter(|update| update.kind == StreamUpdateKind::PermissionDenied)
        .filter_map(|update| update.text.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        denied,
        [
            "Write fixture output",
            "cursor/ask_question",
            "cursor/create_plan"
        ]
    );
    let subagent = result
        .updates
        .iter()
        .find(|update| {
            update.kind == StreamUpdateKind::ToolCall && update.text.as_deref() == Some("subagent")
        })
        .unwrap_or_else(|| panic!("subagent observation"));
    assert!(subagent.changed_files.is_empty());
    assert!(subagent.cost.is_none());
}
