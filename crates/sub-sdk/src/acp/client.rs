//! ACP v1 client over stdio.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
use serde::{Deserialize, Serialize};

use super::config::AcpClientConfig;
use super::error::AcpError;
use super::launch::HarnessLaunch;
use super::session::{PromptResult, SessionHandle, SessionStart};
use super::stop_reason::StopReason;
use super::update::StreamUpdate;

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcRequest)]
#[request(method = "cursor/ask_question", response = CursorExtensionResponse)]
struct CursorAskQuestionRequest(serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcRequest)]
#[request(method = "cursor/create_plan", response = CursorExtensionResponse)]
struct CursorCreatePlanRequest(serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcResponse)]
struct CursorExtensionResponse(serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcNotification)]
#[notification(method = "cursor/task")]
struct CursorTaskNotification(serde_json::Value);

/// Options for one prompt turn.
#[derive(Clone, Default)]
pub struct PromptOptions {
    /// Fail with [`AcpError::TimedOut`] when the turn exceeds this duration.
    pub timeout: Option<Duration>,
    /// Send `session/cancel` after this delay once the prompt starts.
    pub cancel_after: Option<Duration>,
    /// Watch this durable request marker and bound the harness's cancellation grace period.
    pub cancellation: Option<CancellationOptions>,
    /// Harness-native ACP mode identifier applied before prompting.
    pub permission_mode: Option<String>,
    /// Harness-native model identifier applied before prompting.
    pub model: Option<String>,
    /// Bridge-specific `_meta` attached to `session/new`.
    pub session_meta: Option<serde_json::Value>,
    /// Create, resume, or replay-load the harness session.
    pub session_start: SessionStart,
    /// Callback notified for each normalized stream update.
    pub update_observer: Option<UpdateObserver>,
    /// Callback notified as soon as the harness session is open.
    pub session_observer: Option<SessionObserver>,
}

impl fmt::Debug for PromptOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PromptOptions")
            .field("timeout", &self.timeout)
            .field("cancel_after", &self.cancel_after)
            .field("cancellation", &self.cancellation)
            .field("permission_mode", &self.permission_mode)
            .field("model", &self.model)
            .field("session_meta", &self.session_meta)
            .field("session_start", &self.session_start)
            .field("update_observer", &self.update_observer.is_some())
            .field("session_observer", &self.session_observer.is_some())
            .finish()
    }
}

/// Supervisor-owned cancellation signal for one prompt turn.
#[derive(Debug, Clone)]
pub struct CancellationOptions {
    /// Durable marker created by another process to request cancellation.
    pub request_path: PathBuf,
    /// Maximum time allowed for the harness to acknowledge `session/cancel`.
    pub grace_period: Duration,
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
        let timeout = options.timeout;
        let run = run_prompt_turn(
            self,
            PromptTurn {
                cwd: cwd.as_ref().to_path_buf(),
                prompt: prompt.to_owned(),
                options,
            },
        );
        match timeout {
            Some(duration) => tokio::time::timeout(duration, run)
                .await
                .map_err(|_| AcpError::TimedOut(duration))?,
            None => run.await,
        }
    }
}

macro_rules! request_handler {
    ($sink:expr, $method:ident) => {{
        let sink = $sink.clone();
        async move |request, responder, _connection| sink.$method(request, responder).await
    }};
}

macro_rules! notification_handler {
    ($sink:expr, $method:ident) => {{
        let sink = $sink.clone();
        async move |notification, _connection| sink.$method(notification).await
    }};
}

struct PromptTurn {
    cwd: PathBuf,
    prompt: String,
    options: PromptOptions,
}

#[derive(Clone)]
struct StreamSink {
    updates: tokio::sync::mpsc::UnboundedSender<StreamUpdate>,
    observer: Option<UpdateObserver>,
    loading_replay: Arc<AtomicBool>,
}

impl StreamSink {
    fn send(&self, update: StreamUpdate) -> Result<(), agent_client_protocol::Error> {
        if let Some(observer) = &self.observer {
            observer(update.clone());
        }
        self.updates
            .send(update)
            .map_err(|_| agent_client_protocol::Error::internal_error())
    }

    #[allow(clippy::unused_async)]
    async fn permission(
        &self,
        request: RequestPermissionRequest,
        responder: agent_client_protocol::Responder<RequestPermissionResponse>,
    ) -> Result<(), agent_client_protocol::Error> {
        self.send(StreamUpdate::permission_denied(&request))?;
        deny_permission(responder)
    }

    #[allow(clippy::unused_async)]
    async fn cursor_question(
        &self,
        _request: CursorAskQuestionRequest,
        responder: agent_client_protocol::Responder<CursorExtensionResponse>,
    ) -> Result<(), agent_client_protocol::Error> {
        self.deny_cursor_interaction("ask_question", responder)
    }

    #[allow(clippy::unused_async)]
    async fn cursor_plan(
        &self,
        _request: CursorCreatePlanRequest,
        responder: agent_client_protocol::Responder<CursorExtensionResponse>,
    ) -> Result<(), agent_client_protocol::Error> {
        self.deny_cursor_interaction("create_plan", responder)
    }

    fn deny_cursor_interaction(
        &self,
        kind: &str,
        responder: agent_client_protocol::Responder<CursorExtensionResponse>,
    ) -> Result<(), agent_client_protocol::Error> {
        if !self.loading_replay.load(Ordering::Acquire) {
            self.send(StreamUpdate::cursor_interaction_denied(kind))?;
        }
        responder.respond(cursor_cancelled_response())
    }

    #[allow(clippy::unused_async)]
    async fn session_update(
        &self,
        notification: SessionNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        if self.loading_replay.load(Ordering::Acquire) {
            return Ok(());
        }
        self.send(StreamUpdate::from_session_update(&notification.update))
    }

    #[allow(clippy::unused_async)]
    async fn cursor_task(
        &self,
        _notification: CursorTaskNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        if !self.loading_replay.load(Ordering::Acquire) {
            self.send(StreamUpdate::subagent_observed())?;
        }
        Ok(())
    }
}

struct ConnectionContext {
    turn: PromptTurn,
    result: tokio::sync::oneshot::Sender<(SessionHandle, PromptResult)>,
    updates: tokio::sync::mpsc::UnboundedReceiver<StreamUpdate>,
    loading_replay: Arc<AtomicBool>,
}

async fn run_prompt_turn(
    client: &AcpClient,
    turn: PromptTurn,
) -> Result<(SessionHandle, PromptResult), AcpError> {
    let client_name = client.config.client_name.clone();
    let agent = AcpAgent::new(client.launch.clone().into_acp_config());
    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    let (update_tx, update_rx) = tokio::sync::mpsc::unbounded_channel();
    let loading_replay = Arc::new(AtomicBool::new(matches!(
        &turn.options.session_start,
        SessionStart::Load(_)
    )));
    let sink = StreamSink {
        updates: update_tx,
        observer: turn.options.update_observer.clone(),
        loading_replay: Arc::clone(&loading_replay),
    };
    let context = ConnectionContext {
        turn,
        result: result_tx,
        updates: update_rx,
        loading_replay,
    };
    Client
        .builder()
        .name(&client_name)
        .on_receive_request(
            request_handler!(sink, permission),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            request_handler!(sink, cursor_question),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            request_handler!(sink, cursor_plan),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            notification_handler!(sink, session_update),
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            notification_handler!(sink, cursor_task),
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_with(agent, async move |connection| {
            drive_connection(connection, context).await
        })
        .await
        .map_err(|error| AcpError::protocol(&error))?;
    result_rx.await.map_err(|_| AcpError::StreamEnded)
}

async fn drive_connection(
    connection: ConnectionTo<Agent>,
    mut context: ConnectionContext,
) -> Result<(), agent_client_protocol::Error> {
    connection
        .send_request(InitializeRequest::new(ProtocolVersion::V1))
        .block_task()
        .await?;
    let session_id = open_session(&connection, &context.turn, &context.loading_replay).await?;
    let handle = SessionHandle {
        session_id: session_id.to_string(),
    };
    if let Some(observer) = &context.turn.options.session_observer {
        observer(&handle.session_id);
    }
    configure_session(&connection, &session_id, &context.turn.options).await?;
    schedule_cancel(&connection, &session_id, context.turn.options.cancel_after);
    let (response, cancellation_honored) =
        prompt_agent(&connection, &session_id, &context.turn).await?;
    let result = collect_result(handle, response, cancellation_honored, &mut context.updates);
    context
        .result
        .send(result)
        .map_err(|_| agent_client_protocol::Error::internal_error())
}

async fn open_session(
    connection: &ConnectionTo<Agent>,
    turn: &PromptTurn,
    loading_replay: &AtomicBool,
) -> Result<agent_client_protocol::schema::v1::SessionId, agent_client_protocol::Error> {
    let meta = turn
        .options
        .session_meta
        .as_ref()
        .map(|value| {
            value
                .as_object()
                .cloned()
                .ok_or_else(agent_client_protocol::Error::invalid_params)
        })
        .transpose()?;
    match &turn.options.session_start {
        SessionStart::New => {
            let mut request = NewSessionRequest::new(&turn.cwd);
            if let Some(meta) = meta {
                request = request.meta(meta);
            }
            Ok(connection
                .send_request(request)
                .block_task()
                .await?
                .session_id)
        }
        SessionStart::Resume(session_id) => {
            let mut request = ResumeSessionRequest::new(session_id.clone(), &turn.cwd);
            if let Some(meta) = meta {
                request = request.meta(meta);
            }
            connection.send_request(request).block_task().await?;
            Ok(session_id.clone().into())
        }
        SessionStart::Load(session_id) => {
            let mut request = LoadSessionRequest::new(session_id.clone(), &turn.cwd);
            if let Some(meta) = meta {
                request = request.meta(meta);
            }
            connection.send_request(request).block_task().await?;
            loading_replay.store(false, Ordering::Release);
            Ok(session_id.clone().into())
        }
    }
}

async fn configure_session(
    connection: &ConnectionTo<Agent>,
    session_id: &agent_client_protocol::schema::v1::SessionId,
    options: &PromptOptions,
) -> Result<(), agent_client_protocol::Error> {
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
    Ok(())
}

fn schedule_cancel(
    connection: &ConnectionTo<Agent>,
    session_id: &agent_client_protocol::schema::v1::SessionId,
    delay: Option<Duration>,
) {
    if let Some(delay) = delay {
        let connection = connection.clone();
        let session_id = session_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let _ = connection.send_notification(
                agent_client_protocol::schema::v1::CancelNotification::new(session_id),
            );
        });
    }
}

async fn prompt_agent(
    connection: &ConnectionTo<Agent>,
    session_id: &agent_client_protocol::schema::v1::SessionId,
    turn: &PromptTurn,
) -> Result<
    (
        Option<agent_client_protocol::schema::v1::PromptResponse>,
        Option<bool>,
    ),
    agent_client_protocol::Error,
> {
    let prompt_request = connection
        .send_request(PromptRequest::new(
            session_id.clone(),
            vec![turn.prompt.clone().into()],
        ))
        .block_task();
    tokio::pin!(prompt_request);
    let Some(cancellation) = &turn.options.cancellation else {
        return Ok((Some(prompt_request.await?), None));
    };
    tokio::select! {
        response = &mut prompt_request => Ok((Some(response?), None)),
        () = wait_for_cancel_request(&cancellation.request_path) => {
            connection.send_notification(
                agent_client_protocol::schema::v1::CancelNotification::new(session_id.clone()),
            )?;
            cancellation_response(&mut prompt_request, cancellation.grace_period).await
        }
    }
}

async fn cancellation_response<F>(
    prompt_request: &mut std::pin::Pin<&mut F>,
    grace_period: Duration,
) -> Result<
    (
        Option<agent_client_protocol::schema::v1::PromptResponse>,
        Option<bool>,
    ),
    agent_client_protocol::Error,
>
where
    F: std::future::Future<
            Output = Result<
                agent_client_protocol::schema::v1::PromptResponse,
                agent_client_protocol::Error,
            >,
        >,
{
    match tokio::time::timeout(grace_period, prompt_request).await {
        Ok(response) => {
            let response = response?;
            let honored = StopReason::from(response.stop_reason) == StopReason::Cancelled;
            Ok((Some(response), Some(honored)))
        }
        Err(_) => Ok((None, Some(false))),
    }
}

fn collect_result(
    handle: SessionHandle,
    response: Option<agent_client_protocol::schema::v1::PromptResponse>,
    cancellation_honored: Option<bool>,
    updates_rx: &mut tokio::sync::mpsc::UnboundedReceiver<StreamUpdate>,
) -> (SessionHandle, PromptResult) {
    let mut updates = Vec::new();
    let mut final_text = String::new();
    while let Ok(update) = updates_rx.try_recv() {
        if update.kind == super::update::StreamUpdateKind::AgentMessageChunk
            && let Some(text) = &update.text
        {
            final_text.push_str(text);
        }
        updates.push(update);
    }
    (
        handle,
        PromptResult {
            stop_reason: response
                .as_ref()
                .map_or(StopReason::Cancelled, |value| value.stop_reason.into()),
            updates,
            final_text,
            usage: response.and_then(|value| value.usage.map(Into::into)),
            cancellation_honored,
        },
    )
}

async fn wait_for_cancel_request(path: &Path) {
    while !path.is_file() {
        tokio::time::sleep(Duration::from_millis(25)).await;
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

fn cursor_cancelled_response() -> CursorExtensionResponse {
    CursorExtensionResponse(serde_json::json!({"outcome":{"outcome":"cancelled"}}))
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
        assert!(options.cancellation.is_none());
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
