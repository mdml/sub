use super::*;

#[test]
fn prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_sub-mcp"))
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("failed to run sub-mcp: {e}"));
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        format!("sub-mcp {}", env!("CARGO_PKG_VERSION"))
    );
}
