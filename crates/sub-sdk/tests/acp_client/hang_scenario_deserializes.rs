use super::*;

#[test]
fn hang_scenario_deserializes() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sub-harness-fake/scenarios/hang.scenario.toml");
    let scenario = Scenario::load(path).unwrap_or_else(|error| panic!("scenario: {error}"));
    assert_eq!(scenario.behavior, ScenarioBehavior::Hang);
}
