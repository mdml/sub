use super::*;

#[test]
fn derives_changed_file_from_streamed_markdown_link() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let file = root.path().join("proof.txt");
    fs::write(&file, "proof").unwrap_or_else(|error| panic!("write: {error}"));
    let text = format!("Created [proof.txt]({})", file.display());
    assert_eq!(derive_changed_files(&[], &text, root.path()), vec![file]);
}

#[test]
fn ignores_links_outside_working_directory() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    assert!(derive_changed_files(&[], "[outside](/etc/passwd)", root.path()).is_empty());
}

#[test]
fn derives_relative_and_angle_bracket_links() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let file = root.path().join("relative.txt");
    fs::write(&file, "proof").unwrap_or_else(|error| panic!("write: {error}"));
    assert_eq!(
        derive_changed_files(&[], "[file](<relative.txt>)", root.path()),
        vec![file]
    );
    assert!(markdown_destinations("[broken](<missing").is_empty());
    assert!(markdown_destinations("[broken](missing").is_empty());
}

#[test]
fn status_mapping_covers_terminal_reasons() {
    assert_eq!(status_from_stop(StopReason::EndTurn), TaskStatus::Succeeded);
    assert_eq!(
        status_from_stop(StopReason::Cancelled),
        TaskStatus::Cancelled
    );
    assert_eq!(status_from_stop(StopReason::Refusal), TaskStatus::Failed);
    assert_eq!(status_from_stop(StopReason::MaxTokens), TaskStatus::Failed);
    assert_eq!(
        status_from_stop(StopReason::MaxTurnRequests),
        TaskStatus::Failed
    );
}

#[test]
fn finds_native_session_record_recursively() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let nested = root.path().join("year/month");
    fs::create_dir_all(&nested).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let record = nested.join("session-abc.jsonl");
    fs::write(&record, "{}").unwrap_or_else(|error| panic!("write: {error}"));
    assert_eq!(find_session_record(root.path(), "abc"), Some(record));
    let cursor_record = root.path().join("cursor-session-id");
    fs::create_dir(&cursor_record).unwrap_or_else(|error| panic!("cursor record: {error}"));
    assert_eq!(find_session_record(root.path(), "cursor-session-id"), None);
    assert!(cursor_record.exists());
    assert_eq!(find_session_record(root.path(), "missing"), None);
}

#[test]
fn cursor_usage_support_is_explicitly_absent() {
    assert_eq!(
        Harness::CursorAgent.usage_support(),
        UsageSupport {
            cost: false,
            tokens: false
        }
    );
    assert_eq!(
        serde_json::to_value(Harness::CursorAgent)
            .unwrap_or_else(|error| panic!("serialize cursor: {error}")),
        serde_json::json!("cursor")
    );
    assert_eq!(
        serde_json::from_value::<Harness>(serde_json::json!("cursor_agent"))
            .unwrap_or_else(|error| panic!("deserialize legacy cursor: {error}")),
        Harness::CursorAgent
    );
}

#[test]
fn native_fallback_uses_each_harness_name() {
    let cwd = Path::new("/path/that/does/not/exist");
    assert!(native_session_reference(Harness::Claude, cwd, "missing-claude").contains("claude"));
    assert!(native_session_reference(Harness::Codex, cwd, "missing-codex").contains("codex"));
    assert!(
        native_session_reference(Harness::CursorAgent, cwd, "missing-cursor").contains("cursor")
    );
}

#[test]
fn detects_subagent_tool_titles_only() {
    let update = StreamUpdate {
        kind: crate::acp::StreamUpdateKind::ToolCall,
        text: Some("spawn_agent".to_owned()),
        changed_files: Vec::new(),
        cost: None,
    };
    assert!(looks_like_subagent(&update));
    let ordinary = StreamUpdate {
        kind: crate::acp::StreamUpdateKind::AgentMessageChunk,
        text: Some("subagent".to_owned()),
        changed_files: Vec::new(),
        cost: None,
    };
    assert!(!looks_like_subagent(&ordinary));
}

#[test]
fn activity_mapping_covers_the_normalized_vocabulary() {
    let cases = [
        (StreamUpdateKind::AgentMessageChunk, ActivityKind::Message),
        (StreamUpdateKind::AgentThoughtChunk, ActivityKind::Thought),
        (StreamUpdateKind::ToolCall, ActivityKind::ToolCall),
        (
            StreamUpdateKind::ToolCallUpdate,
            ActivityKind::ToolCallUpdate,
        ),
        (
            StreamUpdateKind::SessionInfoUpdate,
            ActivityKind::SessionInfo,
        ),
        (
            StreamUpdateKind::AvailableCommandsUpdate,
            ActivityKind::AvailableCommands,
        ),
        (StreamUpdateKind::Plan, ActivityKind::Plan),
        (
            StreamUpdateKind::PermissionDenied,
            ActivityKind::PermissionDenied,
        ),
        (StreamUpdateKind::Other, ActivityKind::Other),
    ];
    for (update, expected) in cases {
        assert_eq!(activity_kind(update, false), Some(expected));
    }
    assert_eq!(activity_kind(StreamUpdateKind::UsageUpdate, false), None);
    assert_eq!(
        activity_kind(StreamUpdateKind::ToolCall, true),
        Some(ActivityKind::SubagentObserved)
    );
}
