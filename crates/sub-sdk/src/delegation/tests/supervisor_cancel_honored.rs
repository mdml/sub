use super::*;

#[tokio::test]
async fn supervisor_delivers_cancel_and_preserves_partial_result() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_454545454545454545454545".to_owned(),
    };
    let paths = prepare_supervisor(
        root.path(),
        &handle,
        &fake_request(root.path(), "cancel_honored"),
    );
    let request: SupervisorRequest =
        read_json(&paths.request).unwrap_or_else(|error| panic!("request: {error}"));
    write_json(
        &paths.task,
        &DelegatedTask {
            handle: handle.clone(),
            params: request.params,
            attempt: ExecutionAttempt {
                number: 1,
                status: TaskStatus::Queued,
                supervisor_pid: Some(std::process::id()),
                supervisor_start_time: process_start_time(std::process::id()),
                harness_session_id: None,
                usage: UsageTotals::default(),
            },
        },
    )
    .unwrap_or_else(|error| panic!("task: {error}"));

    let state_dir = root.path().to_path_buf();
    let supervisor_handle = handle.clone();
    let supervisor =
        tokio::spawn(async move { run_supervisor(&state_dir, &supervisor_handle, 1).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let outcome = Delegator::new(root.path(), "/does/not/run")
        .cancel(&handle)
        .unwrap_or_else(|error| panic!("cancel: {error}"));
    assert_eq!(outcome.delivery, CancelDelivery::Delivered);
    supervisor
        .await
        .unwrap_or_else(|error| panic!("join: {error}"))
        .unwrap_or_else(|error| panic!("supervisor: {error}"));

    let result: TaskResult =
        read_json(&paths.result).unwrap_or_else(|error| panic!("result: {error}"));
    assert_eq!(result.status, TaskStatus::Cancelled);
    assert!(!result.summary.is_empty());
    assert!(result.harness_session_id.is_some());
    assert!(
        result
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == ArtifactKind::NativeSession)
    );
    let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
    assert!(events.iter().any(|event| matches!(
        event.kind,
        TaskEventKind::AttemptCancelled {
            harness_honored: true
        }
    )));
    assert!(events.iter().any(|event| matches!(
        event.kind,
        TaskEventKind::AttemptFinished {
            status: TaskStatus::Cancelled
        }
    )));
}
