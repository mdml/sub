//! ACP v1 client over stdio.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::Agent;
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    InitializeRequest, LoadSessionRequest, NewSessionRequest, PromptRequest,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    ResumeSessionRequest, SessionNotification, SetSessionConfigOptionRequest,
    SetSessionModeRequest,
};
use agent_client_protocol::{AcpAgent, Client, ConnectionTo};

use super::config::AcpClientConfig;
use super::error::AcpError;
use super::launch::HarnessLaunch;
use super::session::{PromptResult, SessionHandle, SessionStart};
use super::stop_reason::StopReason;
use super::update::StreamUpdate;

/// Options for one prompt turn.
#[derive(Debug, Clone, Default)]
pub struct PromptOptions {
    /// Fail with [`AcpError::TimedOut`] when the turn exceeds this duration.
    pub timeout: Option<Duration>,
    /// Send `session/cancel` after this delay once the prompt starts.
    pub cancel_after: Option<Duration>,
    /// Harness-native ACP mode identifier applied before prompting.
    pub permission_mode: Option<String>,
    /// Harness-native model identifier applied before prompting.
    pub model: Option<String>,
    /// Bridge-specific `_meta` attached to `session/new`.
    pub session_meta: Option<serde_json::Value>,
    /// Create, resume, or replay-load the harness session.
    pub session_start: SessionStart,
}

/// Thread-safe callback invoked for each normalized stream update.
pub type UpdateObserver = Arc<dyn Fn(StreamUpdate) + Send + Sync>;

/// Thread-safe callback invoked as soon as the harness session is open.
pub type SessionObserver = Arc<dyn Fn(&str) + Send + Sync>;

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
        self.prompt_turn_observing(cwd, prompt, options, None).await
    }

    /// Run one prompt turn and notify an observer as updates arrive.
    ///
    /// # Errors
    ///
    /// Returns [`AcpError`] when the agent process, negotiation, or turn fails.
    pub async fn prompt_turn_observing(
        &self,
        cwd: impl AsRef<Path>,
        prompt: &str,
        options: PromptOptions,
        observer: Option<UpdateObserver>,
    ) -> Result<(SessionHandle, PromptResult), AcpError> {
        self.prompt_turn_observing_session(cwd, prompt, options, observer, None)
            .await
    }

    /// Run one prompt turn and notify observers as the session opens and updates arrive.
    ///
    /// # Errors
    ///
    /// Returns [`AcpError`] when the agent process, negotiation, session open, or turn fails.
    pub async fn prompt_turn_observing_session(
        &self,
        cwd: impl AsRef<Path>,
        prompt: &str,
        options: PromptOptions,
        observer: Option<UpdateObserver>,
        session_observer: Option<SessionObserver>,
    ) -> Result<(SessionHandle, PromptResult), AcpError> {
        let run = self.run_prompt_turn(cwd, prompt, &options, observer, session_observer);
        match options.timeout {
            Some(duration) => tokio::time::timeout(duration, run)
                .await
                .map_err(|_| AcpError::TimedOut(duration))?,
            None => run.await,
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the ACP connection lifecycle must remain inside one connection callback"
    )]
    async fn run_prompt_turn(
        &self,
        cwd: impl AsRef<Path>,
        prompt: &str,
        options: &PromptOptions,
        observer: Option<UpdateObserver>,
        session_observer: Option<SessionObserver>,
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
                    let observer = observer.clone();
                    async move |request: RequestPermissionRequest,
                                responder,
                                _connection: ConnectionTo<Agent>| {
                        let update = StreamUpdate::permission_denied(&request);
                        if let Some(observer) = &observer {
                            observer(update.clone());
                        }
                        update_tx
                            .send(update)
                            .map_err(|_| agent_client_protocol::Error::internal_error())?;
                        deny_permission(responder)
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                {
                    let observer = observer.clone();
                    async move |notification: SessionNotification,
                                _connection: ConnectionTo<Agent>| {
                        let update = StreamUpdate::from_session_update(&notification.update);
                        if let Some(observer) = &observer {
                            observer(update.clone());
                        }
                        update_tx
                            .send(update)
                            .map_err(|_| agent_client_protocol::Error::internal_error())
                    }
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(agent, async move |connection| {
                connection
                    .send_request(InitializeRequest::new(ProtocolVersion::V1))
                    .block_task()
                    .await?;

                let meta = options
                    .session_meta
                    .as_ref()
                    .map(|value| {
                        value
                            .as_object()
                            .cloned()
                            .ok_or_else(agent_client_protocol::Error::invalid_params)
                    })
                    .transpose()?;
                let session_id = match &options.session_start {
                    SessionStart::New => {
                        let mut request = NewSessionRequest::new(&cwd);
                        if let Some(meta) = meta.clone() {
                            request = request.meta(meta);
                        }
                        connection
                            .send_request(request)
                            .block_task()
                            .await?
                            .session_id
                    }
                    SessionStart::Resume(session_id) => {
                        let mut request = ResumeSessionRequest::new(session_id.clone(), &cwd);
                        if let Some(meta) = meta.clone() {
                            request = request.meta(meta);
                        }
                        connection.send_request(request).block_task().await?;
                        session_id.clone().into()
                    }
                    SessionStart::Load(session_id) => {
                        let mut request = LoadSessionRequest::new(session_id.clone(), &cwd);
                        if let Some(meta) = meta {
                            request = request.meta(meta);
                        }
                        connection.send_request(request).block_task().await?;
                        session_id.clone().into()
                    }
                };
                let handle = SessionHandle {
                    session_id: session_id.to_string(),
                };
                if let Some(observer) = &session_observer {
                    observer(&handle.session_id);
                }

                if let Some(mode) = &options.permission_mode {
                    connection
                        .send_request(SetSessionModeRequest::new(session_id.clone(), mode.clone()))
                        .block_task()
                        .await?;
                }
                if let Some(model) = &options.model {
                    connection
                        .send_request(SetSessionConfigOptionRequest::new(
                            session_id.clone(),
                            "model",
                            model.as_str(),
                        ))
                        .block_task()
                        .await?;
                }

                if let Some(delay) = cancel_after {
                    let connection = connection.clone();
                    let session_id = session_id.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(delay).await;
                        let _ = connection.send_notification(
                            agent_client_protocol::schema::v1::CancelNotification::new(session_id),
                        );
                    });
                }

                let response = connection
                    .send_request(PromptRequest::new(session_id, vec![prompt.into()]))
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
                        usage: response.usage.map(Into::into),
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
