//! Why an agent stopped processing a prompt turn.

use serde::{Deserialize, Serialize};

/// Why an agent stopped processing a prompt turn.
///
/// Mirrors ACP v1 stop reasons without exposing ACP schema types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StopReason {
    /// The turn ended successfully.
    EndTurn,
    /// The turn ended because the agent reached the maximum number of tokens.
    MaxTokens,
    /// The turn ended because the agent reached the maximum number of allowed
    /// agent requests between user turns.
    MaxTurnRequests,
    /// The agent refused to continue.
    Refusal,
    /// The turn was cancelled by the client via `session/cancel`.
    Cancelled,
}

impl From<agent_client_protocol::schema::v1::StopReason> for StopReason {
    fn from(reason: agent_client_protocol::schema::v1::StopReason) -> Self {
        use agent_client_protocol::schema::v1::StopReason as Acp;
        match reason {
            Acp::MaxTokens => Self::MaxTokens,
            Acp::MaxTurnRequests => Self::MaxTurnRequests,
            Acp::Refusal => Self::Refusal,
            Acp::Cancelled => Self::Cancelled,
            _ => Self::EndTurn, // includes EndTurn and future ACP variants
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::v1::StopReason as AcpStopReason;

    #[test]
    fn maps_acp_stop_reasons() {
        assert_eq!(
            StopReason::from(AcpStopReason::MaxTurnRequests),
            StopReason::MaxTurnRequests
        );
        assert_eq!(
            StopReason::from(AcpStopReason::Refusal),
            StopReason::Refusal
        );
        assert_eq!(
            StopReason::from(AcpStopReason::EndTurn),
            StopReason::EndTurn
        );
        assert_eq!(
            StopReason::from(AcpStopReason::MaxTokens),
            StopReason::MaxTokens
        );
    }
}
