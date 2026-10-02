use super::*;

#[test]
fn supervisor_mode_rejects_missing_handle() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let output = sub_mcp_command(root.path())
        .arg("__supervise")
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("handle missing"));
}
