use super::*;

#[test]
fn codex_fixture_is_recorded() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sub-harness-fake/fixtures/codex-hello");
    let fixture = LoadedFixture::load(dir).unwrap_or_else(|error| panic!("fixture: {error}"));
    assert!(matches!(
        fixture.manifest.source,
        FixtureSource::Recorded { .. }
    ));
    assert!(fixture.events.len() > 100);
}
