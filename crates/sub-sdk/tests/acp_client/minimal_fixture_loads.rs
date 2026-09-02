use super::*;

#[test]
fn minimal_fixture_loads() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sub-harness-fake/fixtures/minimal");
    let fixture = LoadedFixture::load(dir).unwrap_or_else(|error| panic!("fixture: {error}"));
    assert!(matches!(fixture.manifest.source, FixtureSource::Synthetic));
    assert_eq!(fixture.manifest.prompt.stop_reason, StopReason::EndTurn);
}
