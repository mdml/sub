//! Smoke test: the `sub` binary runs and reports its version.

use std::process::Command;

#[test]
fn prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_sub"))
        .output()
        .unwrap_or_else(|e| panic!("failed to run sub: {e}"));
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), format!("sub {}", env!("CARGO_PKG_VERSION")));
}
