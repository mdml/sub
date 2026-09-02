use super::*;

#[test]
fn recover_creates_a_sequential_attempt_for_the_recorded_session() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_121212121212121212121212".to_owned(),
    };
    let request = fake_request(root.path(), "replay-minimal");
    let paths = prepare_supervisor(root.path(), &handle, &request);
    let attempt = ExecutionAttempt {
        number: 1,
        status: TaskStatus::Running,
        supervisor_pid: Some(u32::MAX),
        supervisor_start_time: Some(1),
        harness_session_id: Some("fixture-session".to_owned()),
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

    let supervisor = fake_binary();
    let recovered = Delegator::new(root.path(), &supervisor)
        .recover(&handle)
        .unwrap_or_else(|error| panic!("recover: {error}"));
    assert_eq!(recovered.handle, handle);
    assert_eq!(recovered.attempt, 2);
    let next_paths = TaskPaths::for_attempt(root.path(), &handle, 2);
    let next_request: SupervisorRequest =
        read_json(&next_paths.request).unwrap_or_else(|error| panic!("request: {error}"));
    assert_eq!(
        next_request.resume_session_id.as_deref(),
        Some("fixture-session")
    );
    let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, TaskEventKind::AttemptOrphaned))
    );
    let error = Delegator::new(root.path(), supervisor)
        .recover(&handle)
        .err()
        .unwrap_or_else(|| panic!("queued attempt must not be recoverable"));
    assert!(matches!(error, DelegationError::NotOrphaned(_)));
}

#[test]
fn recover_rejects_cancelled_task_and_records_why() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_151515151515151515151515".to_owned(),
    };
    let request = fake_request(root.path(), "replay-minimal");
    let paths = prepare_supervisor(root.path(), &handle, &request);
    let attempt = ExecutionAttempt {
        number: 1,
        status: TaskStatus::Cancelled,
        supervisor_pid: None,
        supervisor_start_time: None,
        harness_session_id: Some("fixture-session".to_owned()),
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

    let error = Delegator::new(root.path(), "/does/not/run")
        .recover(&handle)
        .err()
        .unwrap_or_else(|| panic!("cancelled task must not recover"));
    assert!(matches!(error, DelegationError::NotOrphaned(_)));
    let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
    assert!(events.iter().any(|event| matches!(
        event.kind,
        TaskEventKind::AttemptRecoveryRejected {
            reason: RecoveryRejectionReason::Cancelled
        }
    )));
}
