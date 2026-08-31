//! How to spawn an ACP agent child process.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Command and environment for spawning an ACP agent over stdio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessLaunch {
    command: PathBuf,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

impl HarnessLaunch {
    /// Spawn configuration for the given executable or command name.
    #[must_use]
    pub fn new(command: impl Into<PathBuf>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    /// Append one command-line argument.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Set one environment variable for the child process.
    #[must_use]
    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }

    /// The executable path or command name.
    #[must_use]
    pub fn command(&self) -> &Path {
        &self.command
    }

    /// Command-line arguments passed to the executable.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Environment variables set for the child process.
    #[must_use]
    pub fn environment(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    pub(crate) fn into_acp_config(self) -> agent_client_protocol::AcpAgentConfig {
        let mut config = agent_client_protocol::AcpAgentConfig::new(self.command);
        config = config.args(self.args);
        config = config.envs(self.env);
        config
    }
}
