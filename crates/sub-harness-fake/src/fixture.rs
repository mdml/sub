//! Fixture manifest and event stream types for the fake harness.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use sub_sdk::acp::StopReason;

use crate::FakeHarnessError;

/// Provenance stamp on a fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixtureSource {
    /// Recorded from a real harness or bridge.
    Recorded {
        /// Harness or bridge name (for example `codex` or `@agentclientprotocol/codex-acp`).
        harness: String,
        /// Version string from the recording.
        version: String,
    },
    /// Authored by hand for deterministic tests.
    Synthetic,
}

/// Parsed fixture manifest (`fixture.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureManifest {
    /// Where the fixture came from.
    pub source: FixtureSource,
    /// Agent identity returned from `initialize`.
    pub agent: AgentInfo,
    /// Default session values for replay.
    pub session: SessionDefaults,
    /// Default prompt completion values.
    pub prompt: PromptDefaults,
    /// File name of the JSONL event stream relative to the fixture directory.
    pub events: String,
}

/// Agent identity returned from `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub title: String,
    pub version: String,
    #[serde(default)]
    pub capabilities: BTreeMap<String, serde_json::Value>,
}

/// Default session values for replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDefaults {
    pub session_id: String,
}

/// Default prompt completion values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptDefaults {
    pub stop_reason: StopReason,
    #[serde(default)]
    pub replay_timing: bool,
}

/// One line from a spike-compatible events JSONL file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedEvent {
    /// Optional millisecond offset from the start of the prompt turn.
    #[serde(default)]
    pub t_ms: u64,
    /// Event kind (for example `session/update`).
    pub kind: String,
    /// Notification payload when `kind` is `session/update`.
    #[serde(default)]
    pub notification: Option<serde_json::Value>,
}

/// Loaded fixture ready for replay.
#[derive(Debug, Clone)]
pub struct LoadedFixture {
    /// Parsed manifest.
    pub manifest: FixtureManifest,
    /// Parsed event stream.
    pub events: Vec<RecordedEvent>,
    /// Fixture directory on disk.
    pub dir: PathBuf,
}

impl LoadedFixture {
    /// Load `fixture.toml` and its event stream from `dir`.
    ///
    /// # Errors
    ///
    /// Returns [`FakeHarnessError`] when the manifest or event stream cannot be read or parsed.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, FakeHarnessError> {
        let dir = dir.as_ref().to_path_buf();
        let manifest_path = dir.join("fixture.toml");
        let manifest_text = fs::read_to_string(&manifest_path).map_err(FakeHarnessError::Io)?;
        let manifest: FixtureManifest =
            toml::from_str(&manifest_text).map_err(FakeHarnessError::serialization)?;

        let events_path = dir.join(&manifest.events);
        let events = load_events_jsonl(&events_path)?;

        Ok(Self {
            manifest,
            events,
            dir,
        })
    }
}

fn load_events_jsonl(path: &Path) -> Result<Vec<RecordedEvent>, FakeHarnessError> {
    let text = fs::read_to_string(path).map_err(FakeHarnessError::Io)?;
    let mut events = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: RecordedEvent = serde_json::from_str(line).map_err(|error| {
            FakeHarnessError::serialization(format!("{}:{}: {error}", path.display(), index + 1))
        })?;
        events.push(event);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_reason_serializes_as_snake_case() {
        let value = serde_json::to_string(&StopReason::EndTurn)
            .unwrap_or_else(|error| panic!("serialize: {error}"));
        assert_eq!(value, "\"end_turn\"");
    }

    #[test]
    fn missing_fixture_dir_errors() {
        assert!(matches!(
            LoadedFixture::load("/nonexistent/fixture/path"),
            Err(FakeHarnessError::Io(_))
        ));
    }

    #[test]
    fn invalid_event_json_errors() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        std::fs::write(
            dir.path().join("fixture.toml"),
            "events = \"events.jsonl\"\n\n[source]\nkind = \"synthetic\"\n\n[prompt]\nstop_reason = \"end_turn\"\nreplay_timing = false\n\n[agent]\nname = \"fake\"\ntitle = \"Fake\"\nversion = \"0.0.0\"\n\n[session]\nsession_id = \"s1\"\n",
        )
        .unwrap_or_else(|error| panic!("write manifest: {error}"));
        std::fs::write(dir.path().join("events.jsonl"), "{ not json\n")
            .unwrap_or_else(|error| panic!("write events: {error}"));

        assert!(matches!(
            LoadedFixture::load(dir.path()),
            Err(FakeHarnessError::Serialization(_))
        ));
    }

    #[test]
    fn invalid_manifest_toml_errors() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        std::fs::write(dir.path().join("fixture.toml"), "events = [not valid")
            .unwrap_or_else(|error| panic!("write manifest: {error}"));

        assert!(matches!(
            LoadedFixture::load(dir.path()),
            Err(FakeHarnessError::Serialization(_))
        ));
    }

    #[test]
    fn missing_event_stream_errors() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        std::fs::write(
            dir.path().join("fixture.toml"),
            "events = \"missing.jsonl\"\n\n[source]\nkind = \"synthetic\"\n\n[prompt]\nstop_reason = \"end_turn\"\n\n[agent]\nname = \"fake\"\ntitle = \"Fake\"\nversion = \"0.0.0\"\n\n[session]\nsession_id = \"s1\"\n",
        )
        .unwrap_or_else(|error| panic!("write manifest: {error}"));

        assert!(matches!(
            LoadedFixture::load(dir.path()),
            Err(FakeHarnessError::Io(_))
        ));
    }

    #[test]
    fn skips_blank_lines_in_event_stream() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        std::fs::write(
            dir.path().join("fixture.toml"),
            "events = \"events.jsonl\"\n\n[source]\nkind = \"synthetic\"\n\n[prompt]\nstop_reason = \"end_turn\"\nreplay_timing = false\n\n[agent]\nname = \"fake\"\ntitle = \"Fake\"\nversion = \"0.0.0\"\n\n[session]\nsession_id = \"s1\"\n",
        )
        .unwrap_or_else(|error| panic!("write manifest: {error}"));
        std::fs::write(
            dir.path().join("events.jsonl"),
            "\n{\"t_ms\":0,\"kind\":\"session/update\",\"notification\":{\"update\":{\"agentMessageChunk\":{\"content\":{\"text\":\"hi\"}}}}}\n\n",
        )
        .unwrap_or_else(|error| panic!("write events: {error}"));

        let fixture =
            LoadedFixture::load(dir.path()).unwrap_or_else(|error| panic!("fixture: {error}"));
        assert_eq!(fixture.events.len(), 1);
    }
}
