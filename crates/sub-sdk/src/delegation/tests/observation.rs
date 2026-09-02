use super::*;

#[tokio::test]
async fn observe_reports_normalized_events_and_accumulated_usage() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_666666666666666666666666".to_owned(),
    };
    let request = fake_request(root.path(), "replay-codex");
    let paths = prepare_supervisor(root.path(), &handle, &request);
    write_json(
        &paths.task,
        &DelegatedTask {
            handle: handle.clone(),
            params: request.params.clone(),
            attempt: ExecutionAttempt {
                number: 1,
                status: TaskStatus::Queued,
                supervisor_pid: None,
                supervisor_start_time: None,
                harness_session_id: None,
                usage: UsageTotals::default(),
            },
        },
    )
    .unwrap_or_else(|error| panic!("task: {error}"));

    run_supervisor(root.path(), &handle, 1)
        .await
        .unwrap_or_else(|error| panic!("supervisor: {error}"));
    let delegator = Delegator::new(root.path(), "/does/not/run");
    let observed = delegator
        .inspect(&handle)
        .unwrap_or_else(|error| panic!("inspect: {error}"));

    assert_eq!(observed.task.handle, handle);
    assert_eq!(observed.task.usage_support, UsageSupport::codex());
    assert_eq!(
        observed
            .task
            .usage
            .tokens
            .as_ref()
            .map(|usage| usage.total_tokens),
        Some(16_749)
    );
    assert!(observed.task.usage.cost.is_none());
    assert!(
        observed
            .events
            .iter()
            .any(|event| matches!(event.kind, TaskEventKind::UsageAccumulated { .. }))
    );
    let events_json = serde_json::to_string(&observed.events)
        .unwrap_or_else(|error| panic!("events json: {error}"));
    assert!(
        !events_json.contains("Hello"),
        "events must not copy transcript text"
    );

    let listed = delegator
        .list()
        .unwrap_or_else(|error| panic!("list: {error}"));
    assert_eq!(listed.tasks.len(), 1);
    assert_eq!(listed.tasks[0], observed.task);
}

#[tokio::test]
async fn observe_keeps_unreported_usage_absent() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_777777777777777777777777".to_owned(),
    };
    let request = fake_request(root.path(), "replay-minimal");
    let paths = prepare_supervisor(root.path(), &handle, &request);
    write_json(
        &paths.task,
        &DelegatedTask {
            handle: handle.clone(),
            params: request.params.clone(),
            attempt: ExecutionAttempt {
                number: 1,
                status: TaskStatus::Queued,
                supervisor_pid: None,
                supervisor_start_time: None,
                harness_session_id: None,
                usage: UsageTotals::default(),
            },
        },
    )
    .unwrap_or_else(|error| panic!("task: {error}"));
    run_supervisor(root.path(), &handle, 1)
        .await
        .unwrap_or_else(|error| panic!("supervisor: {error}"));

    let observed = Delegator::new(root.path(), "/does/not/run")
        .inspect(&handle)
        .unwrap_or_else(|error| panic!("inspect: {error}"));
    assert!(observed.task.usage.tokens.is_none());
    assert!(observed.task.usage.cost.is_none());
}

#[tokio::test]
async fn observe_accumulates_streamed_cost_and_terminal_tokens() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_888888888888888888888888".to_owned(),
    };
    let mut request = fake_request(root.path(), "replay-usage");
    request.params.harness = Harness::Claude;
    let paths = prepare_supervisor(root.path(), &handle, &request);
    write_json(
        &paths.task,
        &DelegatedTask {
            handle: handle.clone(),
            params: request.params.clone(),
            attempt: ExecutionAttempt {
                number: 1,
                status: TaskStatus::Queued,
                supervisor_pid: None,
                supervisor_start_time: None,
                harness_session_id: None,
                usage: UsageTotals::default(),
            },
        },
    )
    .unwrap_or_else(|error| panic!("task: {error}"));
    run_supervisor(root.path(), &handle, 1)
        .await
        .unwrap_or_else(|error| panic!("supervisor: {error}"));

    let observed = Delegator::new(root.path(), "/does/not/run")
        .inspect(&handle)
        .unwrap_or_else(|error| panic!("inspect: {error}"));
    assert_eq!(observed.task.usage_support, UsageSupport::claude());
    assert_eq!(
        observed.task.usage.cost,
        Some(UsageCost {
            amount: 0.25,
            currency: "USD".to_owned(),
        })
    );
    assert_eq!(
        observed.task.usage.tokens.map(|usage| usage.total_tokens),
        Some(120)
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|event| matches!(event.kind, TaskEventKind::UsageAccumulated { .. }))
            .count(),
        2
    );
}

#[tokio::test]
async fn observe_reads_live_state_without_the_supervisor() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_999999999999999999999999".to_owned(),
    };
    let mut request = fake_request(root.path(), "replay-usage");
    request.params.harness = Harness::Claude;
    let paths = prepare_supervisor(root.path(), &handle, &request);
    write_json(
        &paths.task,
        &DelegatedTask {
            handle: handle.clone(),
            params: request.params.clone(),
            attempt: ExecutionAttempt {
                number: 1,
                status: TaskStatus::Queued,
                supervisor_pid: None,
                supervisor_start_time: None,
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

    let live = Delegator::new(root.path(), "/does/not/run")
        .inspect(&handle)
        .unwrap_or_else(|error| panic!("live inspect: {error}"));
    assert_eq!(live.task.status, TaskStatus::Running);
    assert!(
        live.events
            .iter()
            .any(|event| matches!(event.kind, TaskEventKind::AttemptStarted))
    );

    supervisor
        .await
        .unwrap_or_else(|error| panic!("join: {error}"))
        .unwrap_or_else(|error| panic!("supervisor: {error}"));
    let complete = Delegator::new(root.path(), "/does/not/run")
        .inspect(&handle)
        .unwrap_or_else(|error| panic!("complete inspect: {error}"));
    assert_eq!(complete.task.status, TaskStatus::Succeeded);
    assert!(complete.task.usage.cost.is_some());
    assert!(complete.task.usage.tokens.is_some());
}

#[test]
fn observe_accumulates_usage_across_attempts() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_aaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
    };
    let request = fake_request(root.path(), "replay-minimal");
    for (number, amount, tokens) in [(1, 0.25, 100), (2, 0.50, 200)] {
        let paths = TaskPaths::for_attempt(root.path(), &handle, number);
        fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
        write_json(
            &paths.state,
            &ExecutionAttempt {
                number,
                status: TaskStatus::Succeeded,
                supervisor_pid: None,
                supervisor_start_time: None,
                harness_session_id: Some(format!("session-{number}")),
                usage: UsageTotals {
                    cost: Some(UsageCost {
                        amount,
                        currency: "USD".to_owned(),
                    }),
                    tokens: Some(TurnUsage {
                        total_tokens: tokens,
                        input_tokens: tokens - 10,
                        output_tokens: 10,
                        thought_tokens: None,
                        cached_read_tokens: None,
                        cached_write_tokens: None,
                    }),
                },
            },
        )
        .unwrap_or_else(|error| panic!("state: {error}"));
    }
    let paths = TaskPaths::new(root.path(), &handle);
    write_json(
        &paths.task,
        &DelegatedTask {
            handle: handle.clone(),
            params: request.params,
            attempt: ExecutionAttempt {
                number: 1,
                status: TaskStatus::Succeeded,
                supervisor_pid: None,
                supervisor_start_time: None,
                harness_session_id: Some("session-1".to_owned()),
                usage: UsageTotals::default(),
            },
        },
    )
    .unwrap_or_else(|error| panic!("task: {error}"));

    let observed = Delegator::new(root.path(), "/does/not/run")
        .inspect(&handle)
        .unwrap_or_else(|error| panic!("inspect: {error}"));
    assert_eq!(observed.attempts.len(), 2);
    assert_eq!(
        observed.task.usage.cost.as_ref().map(|cost| cost.amount),
        Some(0.75)
    );
    assert_eq!(
        observed.task.usage.tokens.map(|usage| usage.total_tokens),
        Some(300)
    );
}
