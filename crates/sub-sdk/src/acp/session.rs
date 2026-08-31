//! Prompt turn outcomes.

use super::stop_reason::StopReason;
use super::update::StreamUpdate;

/// Outcome of one blocking prompt turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptResult {
    /// Why the agent stopped processing the turn.
    pub stop_reason: StopReason,
    /// Normalized session updates observed during the turn.
    pub updates: Vec<StreamUpdate>,
    /// Concatenated assistant message text from the stream.
    pub final_text: String,
}

/// Handle returned immediately after opening a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandle {
    /// The harness-owned session identifier.
    pub session_id: String,
}
