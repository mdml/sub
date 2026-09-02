use super::*;

#[tokio::test]
async fn supervisor_bounds_an_ignored_cancel() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_464646464646464646464646".to_owned(),
    };
    let pid_file = root.path().join("ignored-cancel.pid");
    let request = ignored_cancel_request(root.path(), &pid_file);
    let paths = prepare_supervisor(root.path(), &handle, &request);
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
    let child_identity = wait_for_process_identity(&pid_file).await;
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
    assert_process_is_dead(child_identity);
    let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
    assert!(events.iter().any(|event| matches!(
        event.kind,
        TaskEventKind::AttemptCancelled {
            harness_honored: false
        }
    )));
}

fn ignored_cancel_request(root: &Path, pid_file: &Path) -> SupervisorRequest {
    let mut request = fake_request(root, "ignore-cancel");
    request.adapter.bridge = request
        .adapter
        .bridge
        .env("SUB_FAKE_PID_FILE", pid_file.to_string_lossy());
    request
}

async fn wait_for_process_identity(pid_file: &Path) -> (u32, u64) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !pid_file.is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("fake PID marker timeout"));
    let pid = fs::read_to_string(pid_file)
        .unwrap_or_else(|error| panic!("fake pid: {error}"))
        .parse::<u32>()
        .unwrap_or_else(|error| panic!("fake pid value: {error}"));
    let start_time = process_start_time(pid).unwrap_or_else(|| panic!("fake process identity"));
    (pid, start_time)
}

fn assert_process_is_dead((pid, start_time): (u32, u64)) {
    assert!(!supervisor_is_alive(&ExecutionAttempt {
        number: 1,
        status: TaskStatus::Running,
        supervisor_pid: Some(pid),
        supervisor_start_time: Some(start_time),
        harness_session_id: None,
        usage: UsageTotals::default(),
    }));
}
