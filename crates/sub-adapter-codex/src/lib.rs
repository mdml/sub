//! `sub` adapter for the `codex` harness. Reserved; no adapter-facing code yet.

/// Name of the harness this adapter drives.
pub const HARNESS_NAME: &str = "codex";

#[cfg(test)]
mod tests {
    use super::HARNESS_NAME;

    #[test]
    fn harness_name() {
        assert_eq!(HARNESS_NAME, "codex");
    }
}
