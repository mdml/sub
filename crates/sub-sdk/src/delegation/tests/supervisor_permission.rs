use super::*;

#[tokio::test]
async fn supervisor_surfaces_denied_permission() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_555555555555555555555555".to_owned(),
    };
    let paths = prepare_supervisor(
        root.path(),
        &handle,
        &fake_request(root.path(), "permission-request"),
    );
    run_supervisor(root.path(), &handle, 1)
        .await
        .unwrap_or_else(|error| panic!("supervisor: {error}"));
    let events = fs::read_to_string(paths.events).unwrap_or_else(|error| panic!("events: {error}"));
    assert!(events.contains("permission_denied"));
}
