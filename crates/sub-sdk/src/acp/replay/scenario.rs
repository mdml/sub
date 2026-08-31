//! Scenario scripting for the fake harness.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::super::error::AcpError;

/// Parsed scenario manifest (`*.scenario.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scenario {
    /// Fixture directory name under `fixtures/`.
    pub fixture: String,
    /// Scripted behavior layered on top of the fixture replay.
    #[serde(flatten)]
    pub behavior: ScenarioBehavior,
}

/// Behavior modifiers for fake-harness scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "snake_case")]
pub enum ScenarioBehavior {
    /// Replay the fixture event stream and complete normally.
    Replay,
    /// Accept `initialize` and `session/new`, then never answer `session/prompt`.
    Hang,
    /// Exit the process after emitting this many session updates.
    DieMidStream {
        /// Number of `session/update` notifications to emit before exiting.
        after_events: usize,
    },
    /// Ignore `session/cancel` and continue until the fixture completes.
    IgnoreCancel,
    /// Write invalid JSON to stdout after emitting this many session updates.
    Malformed {
        /// Number of `session/update` notifications to emit first.
        after_events: usize,
    },
    /// Honor `session/cancel` and return `cancelled`.
    CancelHonored,
}

impl Scenario {
    /// Load a scenario file.
    ///
    /// # Errors
    ///
    /// Returns [`AcpError`] when the file cannot be read or parsed.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, AcpError> {
        let text = fs::read_to_string(path.as_ref()).map_err(AcpError::Io)?;
        toml::from_str(&text).map_err(AcpError::serialization)
    }

    /// Resolve the fixture directory from a scenarios root and fixtures root.
    pub fn fixture_dir(&self, fixtures_root: impl AsRef<Path>) -> PathBuf {
        fixtures_root.as_ref().join(&self.fixture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_honored_scenario_deserializes() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sub-harness-fake/scenarios/cancel_honored.scenario.toml");
        let scenario = Scenario::load(path).unwrap_or_else(|error| panic!("scenario: {error}"));
        assert_eq!(scenario.fixture, "minimal-cancelled");
        assert_eq!(scenario.behavior, ScenarioBehavior::Replay);
    }

    #[test]
    fn missing_scenario_file_errors() {
        let Err(error) = Scenario::load("/nonexistent/scenario.toml") else {
            panic!("expected io error");
        };
        assert!(matches!(error, AcpError::Io(_)));
    }

    #[test]
    fn fixture_dir_joins_fixture_name() {
        let scenario = Scenario {
            fixture: "minimal".to_owned(),
            behavior: ScenarioBehavior::Replay,
        };
        let dir = scenario.fixture_dir("/fixtures");
        assert_eq!(dir, PathBuf::from("/fixtures/minimal"));
    }
}
