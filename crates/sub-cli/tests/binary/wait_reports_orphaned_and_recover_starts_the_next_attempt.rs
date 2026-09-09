use std::process::Stdio;

use super::*;

/// Run one `sub` subcommand against the throwaway state directory and parse its JSON output.
fn cli_json(binary: &str, root: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let output = sub_command(binary, root)
        .args(args)
        .args(["--state-dir"])
        .arg(root)
        .output()
        .unwrap_or_else(|error| panic!("{}: {error}", args[0]));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{} json: {error}", args[0]))
}

/// What recover recorded about the orphaned attempt's child.
fn recorded_disposition(binary: &str, root: &std::path::Path, handle: &str) -> String {
    let inspect = cli_json(binary, root, &["inspect", handle]);
    inspect["events"]
        .as_array()
        .unwrap_or_else(|| panic!("events"))
        .iter()
        .find(|event| event["kind"] == "orphaned_child_disposed")
        .unwrap_or_else(|| panic!("recover must record an orphaned-child disposition"))["disposition"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[cfg(unix)]
#[test]
fn wait_reports_orphaned_and_recover_starts_the_next_attempt() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = "tsk_454545454545454545454545";
    let mut orphan = Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn orphan: {error}"));
    prepare_orphaned_task(root.path(), handle);
    record_harness_child(root.path(), handle, orphan.id());
    let binary = env!("CARGO_BIN_EXE_sub");

    let wait = cli_json(
        binary,
        root.path(),
        &["wait", handle, "--timeout-seconds", "0"],
    );
    assert_eq!(wait["state"], "orphaned");
    assert_eq!(wait["status"], "orphaned");

    let recovered = cli_json(binary, root.path(), &["recover", handle]);
    assert_eq!(recovered["handle"]["id"], handle);
    assert_eq!(recovered["attempt"], 2);
    assert!(
        !orphan
            .wait()
            .unwrap_or_else(|error| panic!("orphan wait: {error}"))
            .success(),
        "recover must end the orphaned attempt's child"
    );
    assert_eq!(
        recorded_disposition(binary, root.path(), handle),
        "terminated"
    );

    let complete = cli_json(
        binary,
        root.path(),
        &["wait", handle, "--timeout-seconds", "3"],
    );
    assert_eq!(complete["state"], "complete");
    assert_eq!(complete["result"]["status"], "failed");
    assert!(
        complete["result"]["summary"]
            .as_str()
            .is_some_and(|summary| !summary.is_empty())
    );

    let cancel = cli_json(binary, root.path(), &["cancel", handle]);
    assert_eq!(cancel["handle"]["id"], handle);
    assert_eq!(cancel["attempt"], 2);
    assert_eq!(cancel["delivery"], "already_finished");
}
