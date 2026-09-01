//! Prompt turn outcomes.

use super::stop_reason::StopReason;
use super::update::StreamUpdate;

/// How a fresh ACP process opens the harness session for a prompt turn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SessionStart {
    /// Create a new harness session.
    #[default]
    New,
    /// Resume an existing harness session without transcript replay.
    Resume(String),
    /// Load an existing harness session and accept bridge replay.
    Load(String),
}

/// Token usage reported for one completed prompt turn.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TurnUsage {
    /// Sum of all reported token types.
    pub total_tokens: u64,
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Reasoning tokens, when distinguished by the harness.
    pub thought_tokens: Option<u64>,
    /// Cache-read tokens, when distinguished by the harness.
    pub cached_read_tokens: Option<u64>,
    /// Cache-write tokens, when distinguished by the harness.
    pub cached_write_tokens: Option<u64>,
}

impl From<agent_client_protocol::schema::v1::Usage> for TurnUsage {
    fn from(value: agent_client_protocol::schema::v1::Usage) -> Self {
        Self {
            total_tokens: value.total_tokens,
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            thought_tokens: value.thought_tokens,
            cached_read_tokens: value.cached_read_tokens,
            cached_write_tokens: value.cached_write_tokens,
        }
    }
}

/// Outcome of one blocking prompt turn.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptResult {
    /// Why the agent stopped processing the turn.
    pub stop_reason: StopReason,
    /// Normalized session updates observed during the turn.
    pub updates: Vec<StreamUpdate>,
    /// Concatenated assistant message text from the stream.
    pub final_text: String,
    /// Per-turn token usage, absent when the harness does not report it.
    pub usage: Option<TurnUsage>,
    /// Whether a requested cancellation was acknowledged before the grace period ended.
    pub cancellation_honored: Option<bool>,
}

/// Handle returned immediately after opening a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandle {
    /// The harness-owned session identifier.
    pub session_id: String,
}
