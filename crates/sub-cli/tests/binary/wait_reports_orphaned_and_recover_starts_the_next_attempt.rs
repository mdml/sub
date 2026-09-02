use super::*;

#[cfg(unix)]
#[test]
fn wait_reports_orphaned_and_recover_starts_the_next_attempt() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = "tsk_454545454545454545454545";
    prepare_orphaned_task(root.path(), handle);
    let binary = env!("CARGO_BIN_EXE_sub");

    let wait = Command::new(binary)
        .args(["wait", handle, "--timeout-seconds", "0", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("wait: {error}"));
    assert!(wait.status.success());
    let wait: serde_json::Value =
        serde_json::from_slice(&wait.stdout).unwrap_or_else(|error| panic!("wait json: {error}"));
    assert_eq!(wait["state"], "orphaned");
    assert_eq!(wait["status"], "orphaned");

    let cancel = Command::new(binary)
        .args(["cancel", handle, "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("cancel: {error}"));
    assert!(cancel.status.success());
    let cancel: serde_json::Value = serde_json::from_slice(&cancel.stdout)
        .unwrap_or_else(|error| panic!("cancel json: {error}"));
    assert_eq!(cancel["handle"]["id"], handle);
    assert_eq!(cancel["attempt"], 1);
    assert_eq!(cancel["delivery"], "attempt_orphaned");

    let recover = Command::new(binary)
        .args(["recover", handle, "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("recover: {error}"));
    assert!(
        recover.status.success(),
        "{}",
        String::from_utf8_lossy(&recover.stderr)
    );
    let recovered: serde_json::Value = serde_json::from_slice(&recover.stdout)
        .unwrap_or_else(|error| panic!("recover json: {error}"));
    assert_eq!(recovered["handle"]["id"], handle);
    assert_eq!(recovered["attempt"], 2);

    let complete = Command::new(binary)
        .args(["wait", handle, "--timeout-seconds", "3", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("complete wait: {error}"));
    assert!(complete.status.success());
    let complete: serde_json::Value = serde_json::from_slice(&complete.stdout)
        .unwrap_or_else(|error| panic!("complete json: {error}"));
    assert_eq!(complete["state"], "complete");
    assert_eq!(complete["result"]["status"], "failed");
    assert!(
        complete["result"]["summary"]
            .as_str()
            .is_some_and(|summary| !summary.is_empty())
    );
}
