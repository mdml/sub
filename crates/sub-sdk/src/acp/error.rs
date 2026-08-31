//! Errors from the shared ACP client layer.

use std::time::Duration;

/// An error from the shared ACP client layer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AcpError {
    /// The underlying ACP SDK returned an error.
    #[error("ACP protocol error: {0}")]
    Protocol(String),

    /// The prompt did not finish before the configured timeout.
    #[error("prompt timed out after {0:?}")]
    TimedOut(Duration),

    /// The agent process exited unexpectedly.
    #[error("agent process exited unexpectedly")]
    ProcessExited,

    /// The update stream ended without a prompt response.
    #[error("agent closed the stream before completing the prompt")]
    StreamEnded,

    /// I/O or spawn failure talking to the agent process.
    #[error("agent I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl AcpError {
    pub(crate) fn protocol(error: &agent_client_protocol::Error) -> Self {
        Self::Protocol(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_context() {
        let error = AcpError::TimedOut(Duration::from_secs(1));
        assert!(error.to_string().contains("timed out"));
    }
}
