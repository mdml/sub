//! ACP v1 client over stdio.

use std::path::Path;
use std::time::Duration;

use agent_client_protocol::Agent;
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    InitializeRequest, NewSessionRequest, PromptRequest, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SessionNotification,
};
use agent_client_protocol::{AcpAgent, Client, ConnectionTo};

use super::config::AcpClientConfig;
use super::error::AcpError;
use super::launch::HarnessLaunch;
use super::session::{PromptResult, SessionHandle};
use super::stop_reason::StopReason;
use super::update::StreamUpdate;

/// Options for one prompt turn.
#[derive(Debug, Clone, Default)]
pub struct PromptOptions {
    /// Fail with [`AcpError::TimedOut`] when the turn exceeds this duration.
    pub timeout: Option<Duration>,
    /// Send `session/cancel` after this delay once the prompt starts.
    pub cancel_after: Option<Duration>,
}

/// Configuration for driving one ACP agent process.
#[derive(Debug, Clone)]
pub struct AcpClient {
    launch: HarnessLaunch,
    config: AcpClientConfig,
}

impl AcpClient {
    /// Create a client configuration for the given agent launch command.
    #[must_use]
    pub fn new(launch: HarnessLaunch, config: AcpClientConfig) -> Self {
        Self { launch, config }
    }

    /// Spawn an agent, open a session, run one prompt turn, and consume the update stream.
    ///
    /// # Errors
    ///
    /// Returns [`AcpError`] when the agent process, protocol negotiation, or prompt turn fails.
    pub async fn prompt_turn(
        &self,
        cwd: impl AsRef<Path>,
        prompt: &str,
        options: PromptOptions,
    ) -> Result<(SessionHandle, PromptResult), AcpError> {
        let run = self.run_prompt_turn(cwd, prompt, &options);
        match options.timeout {
            Some(duration) => tokio::time::timeout(duration, run)
                .await
                .map_err(|_| AcpError::TimedOut(duration))?,
            None => run.await,
        }
    }

    async fn run_prompt_turn(
        &self,
        cwd: impl AsRef<Path>,
        prompt: &str,
        options: &PromptOptions,
    ) -> Result<(SessionHandle, PromptResult), AcpError> {
        let cwd = cwd.as_ref().to_path_buf();
        let prompt = prompt.to_owned();
        let cancel_after = options.cancel_after;
        let client_name = self.config.client_name.clone();
        let agent = AcpAgent::new(self.launch.clone().into_acp_config());
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let (update_tx, mut update_rx) = tokio::sync::mpsc::unbounded_channel();

        Client
            .builder()
            .name(&client_name)
            .on_receive_request(
                {
                    let update_tx = update_tx.clone();
                    async move |request: RequestPermissionRequest,
                                responder,
                                _connection: ConnectionTo<Agent>| {
                        update_tx
                            .send(StreamUpdate::permission_denied(&request))
                            .map_err(|_| agent_client_protocol::Error::internal_error())?;
                        deny_permission(responder)
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                async move |notification: SessionNotification, _connection: ConnectionTo<Agent>| {
                    update_tx
                        .send(StreamUpdate::from_session_update(&notification.update))
                        .map_err(|_| agent_client_protocol::Error::internal_error())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(agent, async move |connection| {
                connection
                    .send_request(InitializeRequest::new(ProtocolVersion::V1))
                    .block_task()
                    .await?;

                let session = connection
                    .send_request(NewSessionRequest::new(&cwd))
                    .block_task()
                    .await?;

                let handle = SessionHandle {
                    session_id: session.session_id.to_string(),
                };

                if let Some(delay) = cancel_after {
                    let connection = connection.clone();
                    let session_id = session.session_id.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(delay).await;
                        let _ = connection.send_notification(
                            agent_client_protocol::schema::v1::CancelNotification::new(session_id),
                        );
                    });
                }

                let response = connection
                    .send_request(PromptRequest::new(session.session_id, vec![prompt.into()]))
                    .block_task()
                    .await?;

                let mut updates = Vec::new();
                let mut final_text = String::new();
                while let Ok(update) = update_rx.try_recv() {
                    if update.kind == super::update::StreamUpdateKind::AgentMessageChunk
                        && let Some(text) = &update.text
                    {
                        final_text.push_str(text);
                    }
                    updates.push(update);
                }
                let turn_result = (
                    handle,
                    PromptResult {
                        stop_reason: StopReason::from(response.stop_reason),
                        updates,
                        final_text,
                    },
                );

                result_tx
                    .send(turn_result)
                    .map_err(|_| agent_client_protocol::Error::internal_error())?;

                Ok(())
            })
            .await
            .map_err(|error| AcpError::protocol(&error))?;

        result_rx.await.map_err(|_| AcpError::StreamEnded)
    }
}

fn deny_permission(
    responder: agent_client_protocol::Responder<RequestPermissionResponse>,
) -> Result<(), agent_client_protocol::Error> {
    responder.respond(permission_response())
}

fn permission_response() -> RequestPermissionResponse {
    RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled)
}

#[cfg(test)]
mod tests {
    use agent_client_protocol::schema::v1::{
        RequestPermissionRequest, SessionId, ToolCallId, ToolCallUpdate, ToolCallUpdateFields,
    };

    use super::*;

    #[test]
    fn prompt_options_default_has_no_timeout_or_cancel() {
        let options = PromptOptions::default();
        assert!(options.timeout.is_none());
        assert!(options.cancel_after.is_none());
    }

    fn sample_permission_request() -> RequestPermissionRequest {
        RequestPermissionRequest::new(
            SessionId::new("session"),
            ToolCallUpdate::new(
                ToolCallId::new("tool"),
                ToolCallUpdateFields::default().title("Run a command"),
            ),
            Vec::new(),
        )
    }

    #[test]
    fn permission_denial_update_names_tool_call() {
        let update = StreamUpdate::permission_denied(&sample_permission_request());
        assert_eq!(
            update.kind,
            super::super::update::StreamUpdateKind::PermissionDenied
        );
        assert_eq!(update.text.as_deref(), Some("Run a command"));
    }

    #[test]
    fn permission_response_is_always_cancelled() {
        assert_eq!(
            permission_response().outcome,
            RequestPermissionOutcome::Cancelled
        );
    }
}
