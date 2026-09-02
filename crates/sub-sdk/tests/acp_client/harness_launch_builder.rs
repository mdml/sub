use super::*;

#[test]
fn harness_launch_builder() {
    let launch = HarnessLaunch::new("sub-harness-fake")
        .arg("replay-minimal")
        .env("SUB_FAKE_FIXTURES_DIR", "/tmp/fixtures");
    assert_eq!(launch.command(), std::path::Path::new("sub-harness-fake"));
    assert_eq!(launch.args(), &["replay-minimal".to_owned()]);
    assert_eq!(
        launch.environment().get("SUB_FAKE_FIXTURES_DIR"),
        Some(&"/tmp/fixtures".to_owned())
    );
}
