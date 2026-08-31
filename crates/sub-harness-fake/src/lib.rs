//! Programmable fake harness for the behavioral contract suite.
//!
//! Binary entry point: `sub-harness-fake`. Library helpers live in
//! [`sub_sdk::acp::replay`].

use std::env;
use std::path::PathBuf;

/// Name under which the fake harness identifies itself.
pub const HARNESS_NAME: &str = "fake";

/// Version string for the fake harness crate.
pub const HARNESS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default scenarios directory bundled with this crate.
#[must_use]
pub fn scenarios_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scenarios")
}

/// Default fixtures directory bundled with this crate.
#[must_use]
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// Run the fake harness using CLI args and environment.
///
/// # Errors
///
/// Returns an error when the scenario is missing, fixtures cannot load, or the
/// ACP server fails.
pub async fn run_from_env() -> Result<(), Box<dyn std::error::Error>> {
    let scenario = scenario_name()?;
    let (scenarios_root, fixtures_root) = resolve_roots();

    sub_sdk::acp::replay::run_stdio(&scenarios_root, &fixtures_root, &scenario).await?;
    Ok(())
}

fn resolve_roots() -> (PathBuf, PathBuf) {
    resolve_roots_from(
        env::var("SUB_FAKE_SCENARIOS_DIR").ok().map(PathBuf::from),
        env::var("SUB_FAKE_FIXTURES_DIR").ok().map(PathBuf::from),
    )
}

fn resolve_roots_from(
    scenarios_override: Option<PathBuf>,
    fixtures_override: Option<PathBuf>,
) -> (PathBuf, PathBuf) {
    let scenarios_root = scenarios_override.unwrap_or_else(scenarios_dir);
    let fixtures_root = fixtures_override.unwrap_or_else(fixtures_dir);
    (scenarios_root, fixtures_root)
}

fn scenario_name() -> Result<String, Box<dyn std::error::Error>> {
    parse_scenario_name(env::args().skip(1))
}

fn parse_scenario_name(
    mut args: impl Iterator<Item = String>,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(flag) = args.next() {
        if flag == "--scenario" {
            if let Some(name) = args.next() {
                return Ok(name);
            }
            return Err("--scenario requires a value".into());
        }
        return Ok(flag);
    }

    env::var("SUB_FAKE_SCENARIO")
        .map_err(|_| "set SUB_FAKE_SCENARIO or pass --scenario <name>".into())
}

#[cfg(test)]
mod tests {
    use super::{
        HARNESS_NAME, fixtures_dir, parse_scenario_name, resolve_roots_from, scenarios_dir,
    };
    use std::path::PathBuf;

    #[test]
    fn harness_name_is_fake() {
        assert_eq!(HARNESS_NAME, "fake");
    }

    #[test]
    fn bundled_dirs_exist() {
        assert!(scenarios_dir().is_dir());
        assert!(fixtures_dir().is_dir());
    }

    #[test]
    fn parse_scenario_name_accepts_positional() {
        let name = parse_scenario_name(["replay-minimal".to_owned()].into_iter())
            .unwrap_or_else(|error| panic!("scenario: {error}"));
        assert_eq!(name, "replay-minimal");
    }

    #[test]
    fn parse_scenario_name_accepts_flag_form() {
        let name = parse_scenario_name(["--scenario".to_owned(), "hang".to_owned()].into_iter())
            .unwrap_or_else(|error| panic!("scenario: {error}"));
        assert_eq!(name, "hang");
    }

    #[test]
    fn parse_scenario_name_rejects_missing_flag_value() {
        let error = parse_scenario_name(["--scenario".to_owned()].into_iter())
            .unwrap_or_else(|error| error.to_string());
        assert!(error.contains("requires a value"));
    }

    #[test]
    fn resolve_roots_defaults_to_bundled_dirs() {
        let (scenarios, fixtures) = resolve_roots_from(None, None);
        assert_eq!(scenarios, scenarios_dir());
        assert_eq!(fixtures, fixtures_dir());
    }

    #[test]
    fn resolve_roots_honors_overrides() {
        let (scenarios, fixtures) = resolve_roots_from(
            Some(PathBuf::from("/tmp/scenarios")),
            Some(PathBuf::from("/tmp/fixtures")),
        );
        assert_eq!(scenarios, PathBuf::from("/tmp/scenarios"));
        assert_eq!(fixtures, PathBuf::from("/tmp/fixtures"));
    }
}
