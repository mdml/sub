use std::process::Stdio;

use super::*;

fn run_json(
    binary: &str,
    command: &str,
    handle: &str,
    root: &std::path::Path,
) -> serde_json::Value {
    let output = Command::new(binary)
        .args([command, handle, "--state-dir"])
        .arg(root)
        .output()
        .unwrap_or_else(|error| panic!("{command}: {error}"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{command} json: {error}"))
}

fn event_kinds(inspect: &serde_json::Value) -> Vec<String> {
    inspect["events"]
        .as_array()
        .unwrap_or_else(|| panic!("events"))
        .iter()
        .map(|event| event["kind"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[cfg(unix)]
#[test]
fn cancel_ends_an_orphaned_task_and_its_child() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = "tsk_474747474747474747474747";
    let mut child = Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn child: {error}"));
    prepare_orphaned_task(root.path(), handle);
    record_harness_child(root.path(), handle, child.id());
    let binary = env!("CARGO_BIN_EXE_sub");

    let cancel = run_json(binary, "cancel", handle, root.path());

    assert_eq!(cancel["handle"]["id"], handle);
    assert_eq!(cancel["attempt"], 1);
    assert_eq!(cancel["delivery"], "attempt_orphaned");
    assert!(
        !child
            .wait()
            .unwrap_or_else(|error| panic!("child wait: {error}"))
            .success()
    );
    let complete = Command::new(binary)
        .args(["wait", handle, "--timeout-seconds", "0", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("wait: {error}"));
    let complete: serde_json::Value = serde_json::from_slice(&complete.stdout)
        .unwrap_or_else(|error| panic!("wait json: {error}"));
    assert_eq!(complete["state"], "complete");
    assert_eq!(complete["result"]["status"], "cancelled");
    let inspect = run_json(binary, "inspect", handle, root.path());
    assert_eq!(
        event_kinds(&inspect),
        [
            "attempt_orphaned",
            "orphaned_child_disposed",
            "attempt_cancelled",
            "attempt_finished"
        ]
    );
    assert_eq!(inspect["events"][1]["disposition"], "terminated");
    let recover = Command::new(binary)
        .args(["recover", handle, "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("recover: {error}"));
    assert!(!recover.status.success());
    assert!(String::from_utf8_lossy(&recover.stderr).contains("not orphaned"));
}
