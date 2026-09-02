use super::*;

#[test]
fn command_errors_are_actionable() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let binary = env!("CARGO_BIN_EXE_sub");
    let unsupported = Command::new(binary)
        .args(["bridge", "install", "unknown", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!unsupported.status.success());
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("unsupported harness"));

    let unknown = Command::new(binary)
        .args([
            "wait",
            "tsk_000000000000000000000000",
            "--timeout-seconds",
            "0",
            "--state-dir",
        ])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown task handle"));

    let recover = Command::new(binary)
        .args(["recover", "tsk_000000000000000000000000", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!recover.status.success());
    assert!(String::from_utf8_lossy(&recover.stderr).contains("unknown task handle"));

    let cancel = Command::new(binary)
        .args(["cancel", "tsk_000000000000000000000000", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!cancel.status.success());
    assert!(String::from_utf8_lossy(&cancel.stderr).contains("unknown task handle"));

    let incomplete = Command::new(binary)
        .arg("launch")
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!incomplete.status.success());
    assert!(String::from_utf8_lossy(&incomplete.stderr).contains("--harness is required"));

    for args in [
        vec!["bridge", "install"],
        vec!["wait"],
        vec!["not-a-command"],
    ] {
        let output = Command::new(binary)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run: {error}"));
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("usage"));
    }
}
