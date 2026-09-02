use super::*;

#[tokio::test]
async fn repeated_wait_reads_same_result() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_111111111111111111111111".to_owned(),
    };
    let paths = TaskPaths::new(root.path(), &handle);
    fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
    write_json(
        &paths.state,
        &ExecutionAttempt {
            number: 1,
            status: TaskStatus::Succeeded,
            supervisor_pid: None,
            supervisor_start_time: None,
            harness_session_id: Some("session".to_owned()),
            usage: UsageTotals::default(),
        },
    )
    .unwrap_or_else(|error| panic!("state: {error}"));
    let result = TaskResult {
        status: TaskStatus::Succeeded,
        summary: "done".to_owned(),
        changed_files: Vec::new(),
        artifacts: Vec::new(),
        harness_session_id: Some("session".to_owned()),
    };
    write_json(&paths.result, &result).unwrap_or_else(|error| panic!("result: {error}"));
    let delegator = Delegator::new(root.path(), "/does/not/run");
    for _ in 0..2 {
        let outcome = delegator
            .wait(&handle, Duration::ZERO)
            .await
            .unwrap_or_else(|error| panic!("wait: {error}"));
        assert_eq!(
            outcome,
            WaitOutcome::Complete {
                result: result.clone()
            }
        );
    }
}

#[tokio::test]
async fn wait_returns_running_after_timeout() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_222222222222222222222222".to_owned(),
    };
    let paths = TaskPaths::new(root.path(), &handle);
    fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
    write_json(
        &paths.state,
        &ExecutionAttempt {
            number: 1,
            status: TaskStatus::Running,
            supervisor_pid: Some(std::process::id()),
            supervisor_start_time: process_start_time(std::process::id()),
            harness_session_id: None,
            usage: UsageTotals::default(),
        },
    )
    .unwrap_or_else(|error| panic!("state: {error}"));
    let delegator = Delegator::new(root.path(), "/does/not/run");
    let outcome = delegator
        .wait(&handle, Duration::ZERO)
        .await
        .unwrap_or_else(|error| panic!("wait: {error}"));
    assert_eq!(
        outcome,
        WaitOutcome::Running {
            status: TaskStatus::Running
        }
    );
}
