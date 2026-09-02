use super::*;

#[test]
fn launch_validates_paths_and_persists_handle() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let delegator = Delegator::new(root.path(), "/bin/true");
    let mut request = fake_request(root.path(), "replay-minimal");
    request.params.cwd = PathBuf::from("relative");
    assert!(
        delegator
            .launch(request.params.clone(), request.adapter.clone())
            .is_err()
    );
    request.params.cwd = root.path().to_path_buf();
    request.params.harness_binary = PathBuf::from("relative");
    assert!(
        delegator
            .launch(request.params.clone(), request.adapter.clone())
            .is_err()
    );
    request.params.harness_binary = fake_binary();
    let handle = delegator
        .launch(request.params, request.adapter)
        .unwrap_or_else(|error| panic!("launch: {error}"));
    assert!(handle.id.starts_with("tsk_"));
    assert!(TaskPaths::new(root.path(), &handle).request.is_file());
}

#[tokio::test]
async fn unknown_handle_is_rejected() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let error = Delegator::new(root.path(), "/bin/true")
        .wait(
            &TaskHandle {
                id: "tsk_000000000000000000000000".to_owned(),
            },
            Duration::ZERO,
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("unknown handle error"));
    assert!(matches!(error, DelegationError::UnknownHandle(_)));
    let invalid = Delegator::new(root.path(), "/bin/true")
        .wait(
            &TaskHandle {
                id: "../../escape".to_owned(),
            },
            Duration::ZERO,
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("invalid handle"));
    assert!(matches!(invalid, DelegationError::InvalidParams(_)));
}

#[test]
fn liveness_rejects_missing_process_identity() {
    let mut attempt = ExecutionAttempt {
        number: 1,
        status: TaskStatus::Running,
        supervisor_pid: None,
        supervisor_start_time: None,
        harness_session_id: None,
        usage: UsageTotals::default(),
    };
    assert!(!supervisor_is_alive(&attempt));
    attempt.supervisor_pid = Some(std::process::id());
    assert!(!supervisor_is_alive(&attempt));
}

#[test]
fn handle_validation_rejects_wrong_length_and_case() {
    for id in ["tsk_1234", "tsk_AAAAAAAAAAAAAAAAAAAAAAAA"] {
        assert!(matches!(
            validate_handle(&TaskHandle { id: id.to_owned() }),
            Err(DelegationError::InvalidParams(_))
        ));
    }
}

#[test]
fn event_reader_skips_blanks_and_tolerates_only_an_incomplete_tail() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let path = root.path().join("events.jsonl");
    let valid = serde_json::to_string(&TaskEvent {
        timestamp_unix_ms: 1,
        task_id: "tsk_111111111111111111111111".to_owned(),
        attempt: 1,
        kind: TaskEventKind::AttemptStarted,
    })
    .unwrap_or_else(|error| panic!("event: {error}"));
    fs::write(&path, format!("\n{valid}\n{{\"incomplete\""))
        .unwrap_or_else(|error| panic!("write: {error}"));
    assert_eq!(
        read_events(&path)
            .unwrap_or_else(|error| panic!("read: {error}"))
            .len(),
        1
    );
    fs::write(&path, "not json\n").unwrap_or_else(|error| panic!("write: {error}"));
    assert!(matches!(read_events(&path), Err(DelegationError::Json(_))));
}

#[test]
fn attempt_reader_skips_directories_without_state() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_171717171717171717171717".to_owned(),
    };
    fs::create_dir_all(
        root.path()
            .join("tasks")
            .join(&handle.id)
            .join("attempts/1"),
    )
    .unwrap_or_else(|error| panic!("mkdir: {error}"));
    let (attempts, events, usage) =
        read_attempts(root.path(), &handle).unwrap_or_else(|error| panic!("attempts: {error}"));
    assert!(attempts.is_empty());
    assert!(events.is_empty());
    assert_eq!(usage, UsageTotals::default());
}

#[test]
fn task_usage_rejects_paths_without_required_ancestry() {
    assert!(matches!(
        read_task_usage(Path::new("/")),
        Err(DelegationError::InvalidParams(_))
    ));
    assert!(matches!(
        read_task_usage(Path::new("task")),
        Err(DelegationError::InvalidParams(_))
    ));
}

#[test]
fn usage_does_not_merge_different_currencies() {
    let mut total = UsageTotals {
        cost: Some(UsageCost {
            amount: 1.0,
            currency: "USD".to_owned(),
        }),
        tokens: None,
    };
    add_usage(
        &mut total,
        &UsageTotals {
            cost: Some(UsageCost {
                amount: 2.0,
                currency: "EUR".to_owned(),
            }),
            tokens: None,
        },
    );
    assert_eq!(total.cost.as_ref().map(|cost| cost.amount), Some(1.0));
}
