//! Shared ACP client layer for `sub`.
//!
//! This module wraps the official ACP SDK for spawning agents over stdio,
//! negotiating protocol v1, opening sessions, sending prompts, consuming update
//! streams, cancelling turns, and timing out. Adapters and surfaces consume
//! these types rather than depending on ACP schema crates directly.
//!
//! The [`replay`] submodule supports the programmable fake harness.

pub mod client;
pub mod config;
pub mod error;
pub mod launch;
pub mod replay;
pub mod session;
pub mod stop_reason;
pub mod update;

pub use client::{AcpClient, PromptOptions};
pub use config::{AcpClientConfig, PermissionPolicy};
pub use error::AcpError;
pub use launch::HarnessLaunch;
pub use session::{PromptResult, SessionHandle};
pub use stop_reason::StopReason;
pub use update::{StreamUpdate, StreamUpdateKind};
