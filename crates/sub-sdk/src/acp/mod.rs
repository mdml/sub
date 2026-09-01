//! Shared ACP client layer for `sub`.
//!
//! This module wraps the official ACP SDK for spawning agents over stdio,
//! negotiating protocol v1, opening sessions, sending prompts, consuming update
//! streams, cancelling turns, and timing out. Adapters and surfaces consume
//! these types rather than depending on ACP schema crates directly.
//!
pub mod client;
pub mod config;
pub mod error;
pub mod launch;
pub mod session;
pub mod stop_reason;
pub mod update;

pub use client::{AcpClient, PromptOptions, SessionObserver, UpdateObserver};
pub use config::AcpClientConfig;
pub use error::AcpError;
pub use launch::HarnessLaunch;
pub use session::{PromptResult, SessionHandle, SessionStart, TurnUsage};
pub use stop_reason::StopReason;
pub use update::{StreamCost, StreamUpdate, StreamUpdateKind};
