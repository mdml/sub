//! Shared ACP client configuration.

/// Configuration for one ACP client connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpClientConfig {
    /// Client name sent during `initialize`.
    pub client_name: String,
}

impl Default for AcpClientConfig {
    fn default() -> Self {
        Self {
            client_name: "sub".to_owned(),
        }
    }
}
