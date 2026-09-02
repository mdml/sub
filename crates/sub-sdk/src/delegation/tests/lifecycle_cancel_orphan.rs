use super::*;

#[test]
fn cancel_reports_orphaned_without_writing_a_request() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_131313131313131313131313".to_owned(),
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

    let outcome = Delegator::new(root.path(), "/does/not/run")
        .cancel(&handle)
        .unwrap_or_else(|error| panic!("cancel: {error}"));
    assert_eq!(outcome.delivery, CancelDelivery::AttemptOrphaned);
    assert!(!paths.cancel_request.is_file());
}
