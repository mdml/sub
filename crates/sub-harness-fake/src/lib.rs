//! Programmable fake harness for the behavioral contract suite.
//!
//! It will speak the exact interface the adapters expect, replay streams
//! recorded from the real harnesses, and be scriptable to hang, die
//! mid-stream, ignore cancellation, or emit malformed output. Nothing is
//! implemented yet; this crate reserves the place.

/// Name under which the fake harness identifies itself.
pub const HARNESS_NAME: &str = "fake";

#[cfg(test)]
mod tests {
    use super::HARNESS_NAME;

    #[test]
    fn harness_name_is_fake() {
        assert_eq!(HARNESS_NAME, "fake");
    }
}
