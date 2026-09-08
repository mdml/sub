use super::*;

fn prepare_orphaned_task(root: &Path, handle: &TaskHandle) -> TaskPaths {
    let request = fake_request(root, "replay-minimal");
    let paths = prepare_supervisor(root, handle, &request);
    let attempt = ExecutionAttempt {
        number: 1,
        status: TaskStatus::Running,
        supervisor_pid: Some(u32::MAX),
        supervisor_start_time: Some(1),
        harness_session_id: Some("fixture-session".to_owned()),
        harness_child: None,
        usage: UsageTotals::default(),
    };
    write_json(&paths.state, &attempt).unwrap_or_else(|error| panic!("state: {error}"));
    write_json(
        &paths.task,
        &DelegatedTask {
            handle: handle.clone(),
            params: request.params,
            attempt,
        },
    )
    .unwrap_or_else(|error| panic!("task: {error}"));
    paths
}

fn assert_cancelled_result(paths: &TaskPaths) {
    let result: TaskResult =
        read_json(&paths.result).unwrap_or_else(|error| panic!("result: {error}"));
    assert_eq!(result.status, TaskStatus::Cancelled);
    assert_eq!(
        result.harness_session_id.as_deref(),
        Some("fixture-session")
    );
    assert!(result.summary.contains("orphaned"));
    assert!(
        result
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == ArtifactKind::NativeSession)
    );
}

fn assert_terminal_events(paths: &TaskPaths) {
    let kinds = read_events(&paths.events)
        .unwrap_or_else(|error| panic!("events: {error}"))
        .into_iter()
        .map(|event| event.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            TaskEventKind::AttemptOrphaned,
            TaskEventKind::OrphanedChildDisposed {
                disposition: OrphanedChildDisposition::IdentityUnrecorded
            },
            TaskEventKind::AttemptCancelled {
                harness_honored: false
            },
            TaskEventKind::AttemptFinished {
                status: TaskStatus::Cancelled
            },
        ]
    );
}

#[tokio::test]
async fn cancel_makes_an_orphaned_task_terminal_and_rejects_recovery() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_131313131313131313131313".to_owned(),
    };
    let paths = prepare_orphaned_task(root.path(), &handle);
    let delegator = Delegator::new(root.path(), "/does/not/run");

    let outcome = delegator
        .cancel(&handle)
        .unwrap_or_else(|error| panic!("cancel: {error}"));

    assert_eq!(outcome.delivery, CancelDelivery::AttemptOrphaned);
    assert!(!paths.cancel_request.is_file());
    assert_cancelled_result(&paths);
    assert_terminal_events(&paths);
    let waited = delegator
        .wait(&handle, Duration::ZERO)
        .await
        .unwrap_or_else(|error| panic!("wait: {error}"));
    assert!(
        matches!(waited, WaitOutcome::Complete { result } if result.status == TaskStatus::Cancelled)
    );
    let inspected = delegator
        .inspect(&handle)
        .unwrap_or_else(|error| panic!("inspect: {error}"));
    assert_eq!(inspected.task.status, TaskStatus::Cancelled);
    let error = delegator
        .recover(&handle)
        .err()
        .unwrap_or_else(|| panic!("cancelled task must not recover"));
    assert!(matches!(error, DelegationError::NotOrphaned(_)));
    let again = delegator
        .cancel(&handle)
        .unwrap_or_else(|error| panic!("second cancel: {error}"));
    assert_eq!(again.delivery, CancelDelivery::AlreadyFinished);
}
