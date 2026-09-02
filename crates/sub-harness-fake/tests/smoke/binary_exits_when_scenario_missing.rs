use super::*;

#[test]
fn binary_exits_when_scenario_missing() {
    let output = Command::new(harness_binary())
        .env_remove("SUB_FAKE_SCENARIO")
        .output()
        .unwrap_or_else(|error| panic!("run binary: {error}"));
    assert!(!output.status.success());
}
