use super::*;

#[test]
fn report_drafts_scrubbed_issue_without_submission() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = "tsk_111111111111111111111111";
    prepare_orphaned_task(root.path(), handle);
    let task_path = root.path().join("tasks").join(handle).join("task.json");
    let mut task: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&task_path).unwrap_or_else(|error| panic!("read task: {error}")),
    )
    .unwrap_or_else(|error| panic!("task json: {error}"));
    task["params"]["prompt"] = serde_json::json!(format!(
        "Investigate {} on report-host\nsecret prompt detail",
        root.path().display()
    ));
    std::fs::write(
        &task_path,
        serde_json::to_vec(&task).unwrap_or_else(|error| panic!("serialize task: {error}")),
    )
    .unwrap_or_else(|error| panic!("write task: {error}"));

    let output = Command::new(env!("CARGO_BIN_EXE_sub"))
        .args(["report", handle, "--state-dir"])
        .arg(root.path())
        .env("HOME", root.path())
        .env("HOSTNAME", "report-host")
        .output()
        .unwrap_or_else(|error| panic!("run report: {error}"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let draft = String::from_utf8_lossy(&output.stdout);
    assert!(draft.starts_with("gh issue create -R mdml/sub"));
    assert!(draft.contains(handle));
    assert!(draft.contains("Investigate [HOME] on [HOSTNAME]"));
    assert!(draft.contains("sub inspect"));
    assert!(draft.contains("Review and scrub"));
    assert!(draft.contains("--label 'harness:codex'"));
    assert!(!draft.contains(&root.path().display().to_string()));
    assert!(!draft.contains("report-host"));
    assert!(!draft.contains("secret prompt detail"));
}

#[test]
fn report_help_names_scrub_rules() {
    let output = Command::new(env!("CARGO_BIN_EXE_sub"))
        .args(["report", "--help"])
        .output()
        .unwrap_or_else(|error| panic!("run report help: {error}"));
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "home directories",
        "hostnames",
        "first line",
        "user content",
        "never submits",
    ] {
        assert!(help.contains(expected), "missing {expected}: {help}");
    }
}
