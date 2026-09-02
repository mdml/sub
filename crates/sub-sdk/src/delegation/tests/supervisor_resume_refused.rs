use super::*;

#[tokio::test]
async fn resume_attempt_records_harness_refusal() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_efefefefefefefefefefefef".to_owned(),
    };
    let paths = prepare_resume_attempt(
        root.path(),
        &handle,
        fake_request(root.path(), "resume-refused"),
        Some("00000000-0000-0000-0000-000000000001"),
    );

    run_supervisor(root.path(), &handle, 2)
        .await
        .unwrap_or_else(|error| panic!("supervisor: {error}"));

    let result: TaskResult =
        read_json(&paths.result).unwrap_or_else(|error| panic!("result: {error}"));
    assert_eq!(result.status, TaskStatus::Failed);
    let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
    assert!(events.iter().any(|event| matches!(
        event.kind,
        TaskEventKind::AttemptResumeFailed {
            reason: ResumeFailureReason::HarnessRefused
        }
    )));
}
