use super::*;

#[test]
fn cancel_reports_too_late_for_a_terminal_attempt() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_141414141414141414141414".to_owned(),
    };
    let request = fake_request(root.path(), "replay-minimal");
    let paths = prepare_supervisor(root.path(), &handle, &request);
    let attempt = ExecutionAttempt {
        number: 1,
        status: TaskStatus::Succeeded,
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

    let outcome = Delegator::new(root.path(), "/does/not/run")
        .cancel(&handle)
        .unwrap_or_else(|error| panic!("cancel: {error}"));
    assert_eq!(outcome.delivery, CancelDelivery::AlreadyFinished);
    assert!(!paths.cancel_request.is_file());
}
