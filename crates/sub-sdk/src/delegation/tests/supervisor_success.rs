use super::*;

#[tokio::test]
async fn supervisor_derives_success_from_fake_stream() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_333333333333333333333333".to_owned(),
    };
    let paths = prepare_supervisor(
        root.path(),
        &handle,
        &fake_request(root.path(), "replay-minimal"),
    );
    run_supervisor(root.path(), &handle, 1)
        .await
        .unwrap_or_else(|error| panic!("supervisor: {error}"));
    let result: TaskResult =
        read_json(&paths.result).unwrap_or_else(|error| panic!("result: {error}"));
    assert_eq!(result.status, TaskStatus::Succeeded);
    assert!(result.summary.contains("Hello"));
    assert!(result.harness_session_id.is_some());
    assert!(
        fs::read_to_string(paths.events)
            .unwrap_or_else(|error| panic!("events: {error}"))
            .contains("attempt_finished")
    );
}

#[tokio::test]
async fn resume_attempt_succeeds_with_same_session_lineage() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_abababababababababababab".to_owned(),
    };
    let request = fake_request(root.path(), "replay-minimal");
    let session_id = "00000000-0000-0000-0000-000000000001";
    let paths = prepare_resume_attempt(root.path(), &handle, request, Some(session_id));

    run_supervisor(root.path(), &handle, 2)
        .await
        .unwrap_or_else(|error| panic!("supervisor: {error}"));

    let state: ExecutionAttempt =
        read_json(&paths.state).unwrap_or_else(|error| panic!("state: {error}"));
    assert_eq!(state.status, TaskStatus::Succeeded);
    assert_eq!(state.harness_session_id.as_deref(), Some(session_id));
    let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
    assert!(events.iter().any(|event| {
        event.attempt == 2 && matches!(event.kind, TaskEventKind::AttemptResumed)
    }));
}
