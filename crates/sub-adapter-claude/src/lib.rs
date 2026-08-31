//! Adapter configuration for Claude Code through its pinned ACP bridge.

use std::path::{Path, PathBuf};

use sub_sdk::acp::HarnessLaunch;
use sub_sdk::bridge::{self, BridgeError, BridgeSpec};

/// Harness name this adapter supports.
pub const HARNESS_NAME: &str = "claude";
/// Pinned npm bridge package.
pub const BRIDGE_PACKAGE: &str = "@agentclientprotocol/claude-agent-acp";
/// Exact pinned bridge version.
pub const BRIDGE_VERSION: &str = "0.70.0";
/// Claude Code versions exercised by the real-harness contract suite.
pub const VERIFIED_HARNESS_VERSIONS: &[&str] = &["2.1.246", "2.1.251"];

/// Bridge installation identity.
pub const BRIDGE: BridgeSpec = BridgeSpec {
    package: BRIDGE_PACKAGE,
    version: BRIDGE_VERSION,
    binary: "claude-agent-acp",
    harness: HARNESS_NAME,
};

/// Install the pinned Claude ACP bridge.
///
/// # Errors
///
/// Returns an error when npm installation or manifest creation fails.
pub fn install_bridge(state_dir: &Path) -> Result<PathBuf, BridgeError> {
    bridge::install(state_dir, BRIDGE)
}

/// Resolve and configure the bridge to run the user's Claude binary.
///
/// # Errors
///
/// Returns an error when the bridge manifest or integrity hash is invalid.
pub fn launch(state_dir: &Path, harness_binary: &Path) -> Result<HarnessLaunch, BridgeError> {
    let bridge_binary = bridge::verify(state_dir, BRIDGE)?;
    Ok(HarnessLaunch::new(bridge_binary)
        .env(
            "CLAUDE_CODE_EXECUTABLE",
            harness_binary.to_string_lossy().into_owned(),
        )
        .env("CLAUDECODE", ""))
}

/// Claude bridge metadata for session creation.
#[must_use]
pub fn session_meta() -> serde_json::Value {
    serde_json::json!({"claudeCode":{"options":{"disallowedTools":["Task","Agent"]}}})
}

/// Prompt suffix reinforcing the one-level delegation policy.
pub const DELEGATION_GUARD: &str =
    "Do not create or invoke subagents. Complete this bounded task yourself.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_are_exact_and_meta_disables_subagents() {
        assert!(!BRIDGE_VERSION.contains('*'));
        assert_eq!(
            session_meta()["claudeCode"]["options"]["disallowedTools"][0],
            "Task"
        );
    }

    #[test]
    fn missing_bridge_names_install_action() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let error = launch(root.path(), Path::new("/bin/claude"))
            .err()
            .unwrap_or_else(|| panic!("missing bridge"));
        assert!(error.to_string().contains("sub bridge install claude"));
    }
}
