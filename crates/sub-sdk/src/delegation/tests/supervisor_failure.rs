use super::*;

#[tokio::test]
async fn supervisor_persists_failed_result() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_444444444444444444444444".to_owned(),
    };
    let paths = prepare_supervisor(
        root.path(),
        &handle,
        &fake_request(root.path(), "malformed"),
    );
    run_supervisor(root.path(), &handle, 1)
        .await
        .unwrap_or_else(|error| panic!("supervisor state: {error}"));
    let result: TaskResult =
        read_json(&paths.result).unwrap_or_else(|error| panic!("result: {error}"));
    assert_eq!(result.status, TaskStatus::Failed);
    assert!(result.harness_session_id.is_none());
}
