//! `sub` adapter for the `cursor-agent` harness. Reserved; no adapter-facing code yet.

/// Name of the harness this adapter drives.
pub const HARNESS_NAME: &str = "cursor-agent";

#[cfg(test)]
mod tests {
    use super::HARNESS_NAME;

    #[test]
    fn harness_name() {
        assert_eq!(HARNESS_NAME, "cursor-agent");
    }
}
