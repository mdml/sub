use super::*;

#[tokio::test]
async fn wait_returns_orphaned_without_timing_out() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_232323232323232323232323".to_owned(),
    };
    let paths = TaskPaths::new(root.path(), &handle);
    fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
    write_json(
        &paths.state,
        &ExecutionAttempt {
            number: 1,
            status: TaskStatus::Running,
            supervisor_pid: Some(u32::MAX),
            supervisor_start_time: Some(1),
            harness_session_id: Some("fixture-session".to_owned()),
            usage: UsageTotals::default(),
        },
    )
    .unwrap_or_else(|error| panic!("state: {error}"));

    let outcome = Delegator::new(root.path(), "/does/not/run")
        .wait(&handle, Duration::from_mins(1))
        .await
        .unwrap_or_else(|error| panic!("wait: {error}"));
    assert_eq!(
        outcome,
        WaitOutcome::Orphaned {
            status: TaskStatus::Orphaned
        }
    );
}

#[test]
fn inspect_reports_dead_running_supervisor_as_orphaned() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_999999999999999999999999".to_owned(),
    };
    let paths = TaskPaths::new(root.path(), &handle);
    fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
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
            params: LaunchParams {
                harness: Harness::Codex,
                prompt: "probe".to_owned(),
                cwd: root.path().to_path_buf(),
                harness_binary: std::env::current_exe()
                    .unwrap_or_else(|error| panic!("exe: {error}")),
                model: None,
                permission_mode: "agent".to_owned(),
            },
            attempt,
        },
    )
    .unwrap_or_else(|error| panic!("task: {error}"));

    let observed = Delegator::new(root.path(), "/does/not/run")
        .inspect(&handle)
        .unwrap_or_else(|error| panic!("inspect: {error}"));
    assert_eq!(observed.task.status, TaskStatus::Orphaned);
    assert_eq!(observed.attempts[0].status, TaskStatus::Orphaned);
}
