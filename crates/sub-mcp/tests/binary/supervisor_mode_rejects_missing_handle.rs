use super::*;

#[test]
fn supervisor_mode_rejects_missing_handle() {
    let output = Command::new(env!("CARGO_BIN_EXE_sub-mcp"))
        .arg("__supervise")
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("handle missing"));
}
