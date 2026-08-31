//! Errors produced while loading or running the fake harness.

/// A fake-harness fixture, scenario, or protocol error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FakeHarnessError {
    /// A fixture, scenario, or stream file could not be read.
    #[error("fake harness I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A fixture, scenario, or stream could not be parsed.
    #[error("fake harness serialization error: {0}")]
    Serialization(String),
    /// The ACP connection failed.
    #[error("fake harness ACP protocol error: {0}")]
    Protocol(String),
}

impl FakeHarnessError {
    pub(crate) fn serialization(error: impl std::fmt::Display) -> Self {
        Self::Serialization(error.to_string())
    }

    pub(crate) fn protocol(error: &agent_client_protocol::Error) -> Self {
        Self::Protocol(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_error_preserves_context() {
        let source = agent_client_protocol::Error::internal_error().data("connection failed");
        let error = FakeHarnessError::protocol(&source);
        assert!(error.to_string().contains("connection failed"));
    }
}
