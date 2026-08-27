//! The `sub` kernel.
//!
//! This crate will hold the delegated-work model (tasks, execution attempts,
//! events, results, artifacts) and the SDK that the MCP and CLI surfaces
//! consume. Public shapes are proposed in pull requests to `staging` and are
//! not yet defined here.

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
