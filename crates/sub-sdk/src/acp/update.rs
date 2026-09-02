//! Session update events from an agent stream.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

trait ContentBlockExt {
    fn as_text(&self) -> Option<&str>;
}

impl ContentBlockExt for agent_client_protocol::schema::v1::ContentBlock {
    fn as_text(&self) -> Option<&str> {
        use agent_client_protocol::schema::v1::ContentBlock;
        match self {
            ContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        }
    }
}

/// One normalized update from a running prompt turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamUpdate {
    /// Which kind of session update this represents.
    pub kind: StreamUpdateKind,
    /// Text content when the update carries a message or thought chunk.
    pub text: Option<String>,
    /// File locations attached to an edit, delete, or move tool call.
    pub changed_files: Vec<PathBuf>,
    /// Latest cumulative session cost when reported with usage.
    pub cost: Option<StreamCost>,
}

/// Cost reported on an ACP usage update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamCost {
    /// Numeric amount in the reported currency.
    pub amount: f64,
    /// ISO 4217 currency code.
    pub currency: String,
}

/// The kind of activity reported in a session update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StreamUpdateKind {
    /// An assistant message chunk.
    AgentMessageChunk,
    /// An agent thought chunk.
    AgentThoughtChunk,
    /// A tool call started or changed.
    ToolCall,
    /// A tool call status or output changed.
    ToolCallUpdate,
    /// Token or context usage changed.
    UsageUpdate,
    /// Session metadata changed.
    SessionInfoUpdate,
    /// Available slash commands changed.
    AvailableCommandsUpdate,
    /// A plan update.
    Plan,
    /// A harness permission request denied by `sub`.
    PermissionDenied,
    /// Any other or unrecognized update variant.
    Other,
}

impl StreamUpdate {
    pub(crate) fn permission_denied(
        request: &agent_client_protocol::schema::v1::RequestPermissionRequest,
    ) -> Self {
        Self {
            kind: StreamUpdateKind::PermissionDenied,
            text: request.tool_call.fields.title.clone(),
            changed_files: Vec::new(),
            cost: None,
        }
    }

    pub(crate) fn cursor_interaction_denied(kind: &str) -> Self {
        Self {
            kind: StreamUpdateKind::PermissionDenied,
            text: Some(format!("cursor/{kind}")),
            changed_files: Vec::new(),
            cost: None,
        }
    }

    pub(crate) fn subagent_observed() -> Self {
        Self {
            kind: StreamUpdateKind::ToolCall,
            text: Some("subagent".to_owned()),
            changed_files: Vec::new(),
            cost: None,
        }
    }

    pub(crate) fn from_session_update(
        update: &agent_client_protocol::schema::v1::SessionUpdate,
    ) -> Self {
        use agent_client_protocol::schema::v1::SessionUpdate;

        match update {
            SessionUpdate::AgentMessageChunk(chunk) => Self {
                kind: StreamUpdateKind::AgentMessageChunk,
                text: chunk.content.as_text().map(str::to_owned),
                changed_files: Vec::new(),
                cost: None,
            },
            SessionUpdate::AgentThoughtChunk(chunk) => Self {
                kind: StreamUpdateKind::AgentThoughtChunk,
                text: chunk.content.as_text().map(str::to_owned),
                changed_files: Vec::new(),
                cost: None,
            },
            SessionUpdate::ToolCall(tool) => Self {
                kind: StreamUpdateKind::ToolCall,
                text: Some(tool.title.clone()),
                changed_files: changed_locations(tool.kind, &tool.locations),
                cost: None,
            },
            SessionUpdate::ToolCallUpdate(tool) => Self {
                kind: StreamUpdateKind::ToolCallUpdate,
                text: tool.fields.title.clone(),
                changed_files: changed_tool_update_locations(tool),
                cost: None,
            },
            SessionUpdate::UsageUpdate(usage) => Self {
                kind: StreamUpdateKind::UsageUpdate,
                text: None,
                changed_files: Vec::new(),
                cost: usage.cost.as_ref().map(|cost| StreamCost {
                    amount: cost.amount,
                    currency: cost.currency.clone(),
                }),
            },
            SessionUpdate::SessionInfoUpdate(_) => Self {
                kind: StreamUpdateKind::SessionInfoUpdate,
                text: None,
                changed_files: Vec::new(),
                cost: None,
            },
            SessionUpdate::AvailableCommandsUpdate(_) => Self {
                kind: StreamUpdateKind::AvailableCommandsUpdate,
                text: None,
                changed_files: Vec::new(),
                cost: None,
            },
            SessionUpdate::Plan(_) => Self {
                kind: StreamUpdateKind::Plan,
                text: None,
                changed_files: Vec::new(),
                cost: None,
            },
            _ => Self {
                kind: StreamUpdateKind::Other,
                text: None,
                changed_files: Vec::new(),
                cost: None,
            },
        }
    }
}

fn changed_tool_update_locations(
    tool: &agent_client_protocol::schema::v1::ToolCallUpdate,
) -> Vec<PathBuf> {
    let Some(locations) = &tool.fields.locations else {
        return Vec::new();
    };
    tool.fields
        .kind
        .map_or_else(Vec::new, |kind| changed_locations(kind, locations))
}

fn changed_locations(
    kind: agent_client_protocol::schema::v1::ToolKind,
    locations: &[agent_client_protocol::schema::v1::ToolCallLocation],
) -> Vec<PathBuf> {
    use agent_client_protocol::schema::v1::ToolKind;
    if matches!(kind, ToolKind::Edit | ToolKind::Delete | ToolKind::Move) {
        locations
            .iter()
            .map(|location| location.path.clone())
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::v1::{
        ContentBlock, ContentChunk, ImageContent, Plan, SessionUpdate,
    };

    #[test]
    fn maps_fixture_session_update() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sub-harness-fake/fixtures/minimal");
        let fixture = sub_harness_fake::LoadedFixture::load(dir)
            .unwrap_or_else(|error| panic!("fixture: {error}"));
        let event = fixture
            .events
            .iter()
            .find(|event| event.kind == "session/update")
            .unwrap_or_else(|| panic!("session/update event"));
        let notification = event
            .notification
            .as_ref()
            .unwrap_or_else(|| panic!("notification payload"));
        let update: SessionUpdate = serde_json::from_value(
            notification
                .get("update")
                .cloned()
                .unwrap_or_else(|| panic!("update field")),
        )
        .unwrap_or_else(|error| panic!("session update: {error}"));
        let mapped = StreamUpdate::from_session_update(&update);
        assert_eq!(mapped.kind, StreamUpdateKind::AgentMessageChunk);
        assert!(mapped.text.is_some());
    }

    #[test]
    fn maps_codex_tool_and_usage_updates() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sub-harness-fake/fixtures/codex-hello");
        let fixture = sub_harness_fake::LoadedFixture::load(dir)
            .unwrap_or_else(|error| panic!("fixture: {error}"));

        let mut saw_tool = false;
        let mut saw_usage = false;
        for event in &fixture.events {
            if event.kind != "session/update" {
                continue;
            }
            let notification = event
                .notification
                .as_ref()
                .unwrap_or_else(|| panic!("notification payload"));
            let update: SessionUpdate = serde_json::from_value(
                notification
                    .get("update")
                    .cloned()
                    .unwrap_or_else(|| panic!("update field")),
            )
            .unwrap_or_else(|error| panic!("session update: {error}"));
            let mapped = StreamUpdate::from_session_update(&update);
            if mapped.kind == StreamUpdateKind::ToolCall {
                saw_tool = true;
            }
            if mapped.kind == StreamUpdateKind::UsageUpdate {
                saw_usage = true;
            }
        }
        assert!(saw_tool, "codex fixture should include tool call updates");
        assert!(saw_usage, "codex fixture should include usage updates");
    }

    #[test]
    fn maps_plan_and_other_updates() {
        let plan = StreamUpdate::from_session_update(&SessionUpdate::Plan(Plan::new(Vec::new())));
        assert_eq!(plan.kind, StreamUpdateKind::Plan);
        assert!(plan.text.is_none());

        let other = StreamUpdate::from_session_update(&SessionUpdate::UserMessageChunk(
            ContentChunk::new("user input".into()),
        ));
        assert_eq!(other.kind, StreamUpdateKind::Other);
        assert!(other.text.is_none());
    }

    #[test]
    fn non_text_message_chunk_has_no_text() {
        let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Image(
            ImageContent::new("aW1hZ2U=", "image/png"),
        )));
        let mapped = StreamUpdate::from_session_update(&update);
        assert_eq!(mapped.kind, StreamUpdateKind::AgentMessageChunk);
        assert!(mapped.text.is_none());
    }

    #[test]
    fn maps_changed_files_from_tool_update() {
        use agent_client_protocol::schema::v1::{
            ToolCallId, ToolCallLocation, ToolCallUpdate, ToolCallUpdateFields, ToolKind,
        };
        let update = SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
            ToolCallId::new("edit"),
            ToolCallUpdateFields::new()
                .kind(ToolKind::Edit)
                .locations(vec![ToolCallLocation::new("/tmp/changed")]),
        ));
        let mapped = StreamUpdate::from_session_update(&update);
        assert_eq!(mapped.changed_files, [PathBuf::from("/tmp/changed")]);
    }
}
