//! Shared ACP client configuration.

/// How the client answers `session/request_permission` prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum PermissionPolicy {
    /// Select the first `allow*` option, or cancel when none exist.
    #[default]
    AutoApproveFirst,
    /// Always cancel permission requests.
    DenyAll,
}

/// Configuration for one ACP client connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpClientConfig {
    /// Client name sent during `initialize`.
    pub client_name: String,
    /// How permission requests are answered.
    pub permission_policy: PermissionPolicy,
}

impl Default for AcpClientConfig {
    fn default() -> Self {
        Self {
            client_name: "sub".to_owned(),
            permission_policy: PermissionPolicy::default(),
        }
    }
}
