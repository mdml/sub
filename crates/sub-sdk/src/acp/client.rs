//! ACP v1 client over stdio.

use std::path::Path;
use std::time::Duration;

use agent_client_protocol::Agent;
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    InitializeRequest, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionNotification,
};
use agent_client_protocol::util::MatchDispatch;
use agent_client_protocol::{AcpAgent, Client, ConnectionTo, SessionMessage};

use super::config::{AcpClientConfig, PermissionPolicy};
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
        let permission_policy = self.config.permission_policy;
        let client_name = self.config.client_name.clone();
        let agent = AcpAgent::new(self.launch.clone().into_acp_config());
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();

        Client
            .builder()
            .name(&client_name)
            .on_receive_request(
                async move |request: RequestPermissionRequest,
                            responder,
                            _connection: ConnectionTo<Agent>| {
                    respond_to_permission(&request, responder, permission_policy)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(agent, async move |connection| {
                connection
                    .send_request(InitializeRequest::new(ProtocolVersion::V1))
                    .block_task()
                    .await?;

                let turn_result = connection
                    .build_session(&cwd)
                    .block_task()
                    .run_until(async move |mut session| {
                        let handle = SessionHandle {
                            session_id: session.session_id().to_string(),
                        };

                        if let Some(delay) = cancel_after {
                            let connection = session.connection().clone();
                            let session_id = session.session_id().clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(delay).await;
                                let _ = connection.send_notification(
                                    agent_client_protocol::schema::v1::CancelNotification::new(
                                        session_id,
                                    ),
                                );
                            });
                        }

                        session.send_prompt(&prompt)?;

                        let mut updates = Vec::new();
                        let mut final_text = String::new();

                        loop {
                            let message = session.read_update().await?;
                            match message {
                                SessionMessage::SessionMessage(dispatch) => {
                                    MatchDispatch::new(dispatch)
                                        .if_notification(
                                            async |notification: SessionNotification| {
                                                let update = StreamUpdate::from_session_update(
                                                    &notification.update,
                                                );
                                                if update.kind
                                                    == super::update::StreamUpdateKind::AgentMessageChunk
                                                    && let Some(text) = &update.text
                                                {
                                                    final_text.push_str(text);
                                                }
                                                updates.push(update);
                                                Ok(())
                                            },
                                        )
                                        .await
                                        .otherwise_ignore()?;
                                }
                                SessionMessage::StopReason(stop_reason) => {
                                    return Ok((
                                        handle,
                                        PromptResult {
                                            stop_reason: StopReason::from(stop_reason),
                                            updates,
                                            final_text,
                                        },
                                    ));
                                }
                                _ => {}
                            }
                        }
                    })
                    .await?;

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

fn permission_response(
    request: &RequestPermissionRequest,
    policy: PermissionPolicy,
) -> RequestPermissionResponse {
    match policy {
        PermissionPolicy::DenyAll => {
            RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled)
        }
        PermissionPolicy::AutoApproveFirst => {
            let option_id = request
                .options
                .iter()
                .find(|option| option.option_id.0.starts_with("allow"))
                .map(|option| option.option_id.clone());
            if let Some(id) = option_id {
                RequestPermissionResponse::new(RequestPermissionOutcome::Selected(
                    SelectedPermissionOutcome::new(id),
                ))
            } else {
                RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled)
            }
        }
    }
}

fn respond_to_permission(
    request: &RequestPermissionRequest,
    responder: agent_client_protocol::Responder<RequestPermissionResponse>,
    policy: PermissionPolicy,
) -> Result<(), agent_client_protocol::Error> {
    responder.respond(permission_response(request, policy))
}

#[cfg(test)]
mod tests {
    use agent_client_protocol::schema::v1::{
        PermissionOption, PermissionOptionId, PermissionOptionKind, RequestPermissionRequest,
        SessionId, ToolCallId, ToolCallUpdate, ToolCallUpdateFields,
    };

    use super::*;

    #[test]
    fn prompt_options_default_has_no_timeout_or_cancel() {
        let options = PromptOptions::default();
        assert!(options.timeout.is_none());
        assert!(options.cancel_after.is_none());
    }

    fn sample_permission_request(options: Vec<PermissionOption>) -> RequestPermissionRequest {
        RequestPermissionRequest::new(
            SessionId::new("session"),
            ToolCallUpdate::new(ToolCallId::new("tool"), ToolCallUpdateFields::default()),
            options,
        )
    }

    #[test]
    fn deny_all_permission_policy_cancels() {
        let request = sample_permission_request(vec![PermissionOption::new(
            PermissionOptionId::new("allow-once"),
            "Allow once",
            PermissionOptionKind::AllowOnce,
        )]);
        let response = permission_response(&request, PermissionPolicy::DenyAll);
        assert_eq!(response.outcome, RequestPermissionOutcome::Cancelled);
    }

    #[test]
    fn auto_approve_first_selects_allow_option() {
        let request = sample_permission_request(vec![PermissionOption::new(
            PermissionOptionId::new("allow-always"),
            "Allow always",
            PermissionOptionKind::AllowAlways,
        )]);
        let response = permission_response(&request, PermissionPolicy::AutoApproveFirst);
        assert!(matches!(
            response.outcome,
            RequestPermissionOutcome::Selected(_)
        ));
    }

    #[test]
    fn auto_approve_first_cancels_without_allow_option() {
        let request = sample_permission_request(vec![PermissionOption::new(
            PermissionOptionId::new("deny"),
            "Deny",
            PermissionOptionKind::RejectOnce,
        )]);
        let response = permission_response(&request, PermissionPolicy::AutoApproveFirst);
        assert_eq!(response.outcome, RequestPermissionOutcome::Cancelled);
    }
}
