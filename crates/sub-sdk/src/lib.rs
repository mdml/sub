//! The `sub` kernel.
//!
//! This crate holds the delegated-work model (tasks, execution attempts,
//! events, results, artifacts) and the SDK that the MCP and CLI surfaces
//! consume. Public shapes are proposed in pull requests to `staging`.
//!
//! The shared ACP client layer lives in [`acp`]; adapters depend on it rather
//! than on ACP schema types directly.

pub mod acp;
pub mod bridge;
pub mod delegation;

/// The crate version, as compiled from `Cargo.toml`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn version_matches_manifest() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
