//! Adapter configuration for Cursor Agent's native ACP v1 server.

use std::path::{Path, PathBuf};

use sub_sdk::acp::HarnessLaunch;
use sub_sdk::delegation::ResumeMechanism;

/// Name of the harness this adapter drives.
pub const HARNESS_NAME: &str = "cursor";
/// Cursor Agent versions exercised by the ACP spike and real-harness contract suite.
pub const VERIFIED_HARNESS_VERSIONS: &[&str] = &["2026.08.25-3e8eec8"];
/// Cursor reopens an existing session through replaying ACP `session/load`.
pub const RESUME_MECHANISM: ResumeMechanism = ResumeMechanism::Load;
/// Prompt-level enforcement used because Cursor Agent has no subagent switch.
pub const DELEGATION_GUARD: &str =
    "Do not create or invoke subagents. Complete this bounded task yourself.";

/// Describe Cursor Agent's native ACP transport; no bridge is installed.
#[must_use]
pub fn install_bridge(harness_binary: &Path) -> NativeBridge {
    NativeBridge {
        binary: harness_binary.to_path_buf(),
        message: "cursor-agent speaks ACP v1 natively; no bridge installation required",
    }
}

/// A no-op bridge-install result for a harness with native ACP support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBridge {
    /// User-owned Cursor Agent binary that supplies the native ACP server.
    pub binary: PathBuf,
    /// Human-readable explanation of why no bridge was installed.
    pub message: &'static str,
}

/// Configure Cursor Agent to run its built-in ACP server.
#[must_use]
pub fn launch(harness_binary: &Path) -> HarnessLaunch {
    HarnessLaunch::new(harness_binary).arg("acp")
}

/// Cursor Agent has no session-creation metadata side channel used by this adapter.
#[must_use]
pub fn session_meta() -> serde_json::Value {
    serde_json::json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_launch_has_no_bridge_or_side_channel() {
        assert_eq!(HARNESS_NAME, "cursor");
        assert_eq!(
            launch(Path::new("/bin/cursor-agent")).command(),
            PathBuf::from("/bin/cursor-agent")
        );
        assert_eq!(launch(Path::new("/bin/cursor-agent")).args(), &["acp"]);
        assert_eq!(session_meta(), serde_json::json!({}));
        assert_eq!(RESUME_MECHANISM, ResumeMechanism::Load);
        assert!(DELEGATION_GUARD.contains("Do not create"));
        assert!(
            install_bridge(Path::new("/bin/cursor-agent"))
                .message
                .contains("no bridge")
        );
    }
}
