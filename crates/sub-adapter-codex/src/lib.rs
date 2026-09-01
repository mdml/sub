//! Adapter configuration for Codex through its pinned ACP bridge.

use std::path::{Path, PathBuf};
use sub_sdk::acp::HarnessLaunch;
use sub_sdk::bridge::{self, BridgeError, BridgeSpec};
use sub_sdk::delegation::ResumeMechanism;

/// Harness name this adapter supports.
pub const HARNESS_NAME: &str = "codex";
/// Pinned npm bridge package.
pub const BRIDGE_PACKAGE: &str = "@agentclientprotocol/codex-acp";
/// Exact pinned bridge version.
pub const BRIDGE_VERSION: &str = "1.6.2";
/// Codex versions exercised by the real-harness contract suite.
pub const VERIFIED_HARNESS_VERSIONS: &[&str] = &["0.149.1", "0.151.0"];
/// Codex's bridge reopens an existing session through ACP `session/resume`.
pub const RESUME_MECHANISM: ResumeMechanism = ResumeMechanism::Resume;
/// Bridge installation identity.
pub const BRIDGE: BridgeSpec = BridgeSpec {
    package: BRIDGE_PACKAGE,
    version: BRIDGE_VERSION,
    binary: "codex-acp",
    harness: HARNESS_NAME,
};

/// Install the pinned Codex ACP bridge.
///
/// # Errors
/// Returns an error when npm installation or manifest creation fails.
pub fn install_bridge(state_dir: &Path) -> Result<PathBuf, BridgeError> {
    bridge::install(state_dir, BRIDGE)
}

/// Resolve and configure the bridge to run the user's Codex binary.
///
/// # Errors
/// Returns an error when the bridge manifest or integrity hash is invalid.
pub fn launch(state_dir: &Path, harness_binary: &Path) -> Result<HarnessLaunch, BridgeError> {
    let bridge_binary = bridge::verify(state_dir, BRIDGE)?;
    Ok(HarnessLaunch::new(bridge_binary)
        .env("CODEX_PATH", harness_binary.to_string_lossy().into_owned())
        .env("CODEX_CONFIG", r#"{"features":{"multi_agent":false}}"#))
}

/// Codex has no session-creation metadata side channel used by this adapter.
#[must_use]
pub fn session_meta() -> serde_json::Value {
    serde_json::json!({})
}

/// Prompt guard required because Codex's subagent switch is not verified effective.
pub const DELEGATION_GUARD: &str =
    "Do not create or invoke subagents. Complete this bounded task yourself.";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pins_are_exact_and_guard_forbids_subagents() {
        assert!(!BRIDGE_VERSION.contains('*'));
        assert!(DELEGATION_GUARD.contains("Do not create"));
    }

    #[test]
    fn missing_bridge_names_install_action() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let error = launch(root.path(), Path::new("/bin/codex"))
            .err()
            .unwrap_or_else(|| panic!("missing bridge"));
        assert!(error.to_string().contains("sub bridge install codex"));
    }
}
