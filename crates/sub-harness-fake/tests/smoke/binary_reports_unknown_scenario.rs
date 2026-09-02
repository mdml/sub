use super::*;

#[test]
fn binary_reports_unknown_scenario() {
    let output = Command::new(harness_binary())
        .arg("does-not-exist")
        .output()
        .unwrap_or_else(|error| panic!("run binary: {error}"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("I/O error"));
}
