//! Fake ACP agent process that replays fixtures over stdio.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    AgentCapabilities, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PermissionOption,
    PermissionOptionId, PermissionOptionKind, PromptRequest, PromptResponse,
    RequestPermissionOutcome, RequestPermissionRequest, ResumeSessionRequest,
    ResumeSessionResponse, SessionNotification, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, SetSessionModeRequest, SetSessionModeResponse, StopReason,
    ToolCallId, ToolCallUpdate, ToolCallUpdateFields,
};
use agent_client_protocol::{Agent, Client, ConnectionTo, Responder, Stdio};

use crate::fixture::{AgentInfo, LoadedFixture};
use crate::scenario::{Scenario, ScenarioBehavior};
use sub_sdk::acp::StopReason as SubStopReason;

use crate::FakeHarnessError;

/// Run the fake harness agent on stdio using the given scenario and fixture roots.
///
/// # Errors
///
/// Returns [`FakeHarnessError`] when fixtures cannot be loaded or the agent connection fails.
#[expect(
    clippy::too_many_lines,
    reason = "one ACP builder registers the fake's complete protocol surface"
)]
pub async fn run_stdio(
    scenarios_root: &Path,
    fixtures_root: &Path,
    scenario_name: &str,
) -> Result<(), FakeHarnessError> {
    let scenario_path = scenarios_root.join(format!("{scenario_name}.scenario.toml"));
    let scenario = Scenario::load(&scenario_path)?;
    let fixture = LoadedFixture::load(scenario.fixture_dir(fixtures_root))?;
    let state = Arc::new(SharedState {
        behavior: scenario.behavior,
        fixture,
        cancelled: Arc::new(AtomicBool::new(false)),
    });

    Agent
        .builder()
        .name("sub-harness-fake")
        .on_receive_request(
            {
                let state = Arc::clone(&state);
                async move |request: InitializeRequest,
                            responder: Responder<InitializeResponse>,
                            _connection: ConnectionTo<Client>| {
                    responder.respond(state.initialize_response(&request))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let state = Arc::clone(&state);
                async move |request: ResumeSessionRequest,
                            responder: Responder<ResumeSessionResponse>,
                            _connection: ConnectionTo<Client>| {
                    match state.resume_session(&request) {
                        Ok(response) => responder.respond(response),
                        Err(error) => responder.respond_with_error(error),
                    }
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let state = Arc::clone(&state);
                async move |request: LoadSessionRequest,
                            responder: Responder<LoadSessionResponse>,
                            connection: ConnectionTo<Client>| {
                    match state.load_session(&request) {
                        Ok(updates) => {
                            for update in updates {
                                connection.send_notification(update)?;
                            }
                            responder.respond(LoadSessionResponse::new())
                        }
                        Err(error) => responder.respond_with_error(error),
                    }
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |_request: SetSessionModeRequest,
                        responder: Responder<SetSessionModeResponse>,
                        _connection: ConnectionTo<Client>| {
                responder.respond(SetSessionModeResponse::new())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |_request: SetSessionConfigOptionRequest,
                        responder: Responder<SetSessionConfigOptionResponse>,
                        _connection: ConnectionTo<Client>| {
                responder.respond(SetSessionConfigOptionResponse::new(Vec::new()))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let state = Arc::clone(&state);
                async move |request: NewSessionRequest,
                            responder: Responder<NewSessionResponse>,
                            _connection: ConnectionTo<Client>| {
                    responder.respond(state.new_session_response(request))
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let state = Arc::clone(&state);
                async move |request: PromptRequest,
                            responder: Responder<PromptResponse>,
                            connection: ConnectionTo<Client>| {
                    match state.behavior {
                        ScenarioBehavior::Hang => {
                            std::future::pending::<Result<(), agent_client_protocol::Error>>().await
                        }
                        ScenarioBehavior::PermissionRequest => {
                            SharedState::request_permission(request, responder, &connection)
                        }
                        ScenarioBehavior::CancelHonored | ScenarioBehavior::IgnoreCancel => {
                            let prompt_state = Arc::clone(&state);
                            tokio::spawn(async move {
                                let _ = prompt_state
                                    .replay_prompt(request, responder, connection)
                                    .await;
                            });
                            Ok(())
                        }
                        ScenarioBehavior::Replay
                        | ScenarioBehavior::DieMidStream { .. }
                        | ScenarioBehavior::Malformed { .. }
                        | ScenarioBehavior::ResumeRefused => {
                            state.replay_prompt(request, responder, connection).await
                        }
                    }
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            {
                let state = Arc::clone(&state);
                async move |notification: agent_client_protocol::schema::v1::CancelNotification,
                            _connection: ConnectionTo<Client>| {
                    state.record_cancel(&notification);
                    Ok(())
                }
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_to(Stdio::new())
        .await
        .map_err(|error| FakeHarnessError::protocol(&error))
}

struct SharedState {
    behavior: ScenarioBehavior,
    fixture: LoadedFixture,
    cancelled: Arc<AtomicBool>,
}

impl SharedState {
    fn initialize_response(&self, request: &InitializeRequest) -> InitializeResponse {
        let AgentInfo {
            name,
            title,
            version,
            ..
        } = &self.fixture.manifest.agent;

        InitializeResponse::new(request.protocol_version)
            .agent_info(
                agent_client_protocol::schema::v1::Implementation::new(
                    name.clone(),
                    version.clone(),
                )
                .title(title.clone()),
            )
            .agent_capabilities(AgentCapabilities::new())
    }

    fn new_session_response(&self, _request: NewSessionRequest) -> NewSessionResponse {
        NewSessionResponse::new(self.fixture.manifest.session.session_id.clone())
    }

    fn record_cancel(&self, notification: &agent_client_protocol::schema::v1::CancelNotification) {
        if notification.session_id.0.as_ref() == self.fixture.manifest.session.session_id {
            self.cancelled.store(true, Ordering::SeqCst);
        }
    }

    fn session_available(&self, session_id: &agent_client_protocol::schema::v1::SessionId) -> bool {
        !matches!(self.behavior, ScenarioBehavior::ResumeRefused)
            && session_id.0.as_ref() == self.fixture.manifest.session.session_id
    }

    fn resume_session(
        &self,
        request: &ResumeSessionRequest,
    ) -> Result<ResumeSessionResponse, agent_client_protocol::Error> {
        if self.session_available(&request.session_id) {
            Ok(ResumeSessionResponse::new())
        } else {
            Err(session_unavailable())
        }
    }

    fn load_session(
        &self,
        request: &LoadSessionRequest,
    ) -> Result<Vec<SessionNotification>, agent_client_protocol::Error> {
        if !self.session_available(&request.session_id) {
            return Err(session_unavailable());
        }
        self.fixture
            .events
            .iter()
            .filter(|event| event.kind == "session/update")
            .filter_map(|event| event.notification.as_ref())
            .map(parse_notification)
            .collect()
    }

    fn prompt_response(&self) -> PromptResponse {
        let mut response = PromptResponse::new(self.stop_reason());
        if let Some(usage) = &self.fixture.manifest.prompt.usage {
            response = response.usage(
                agent_client_protocol::schema::v1::Usage::new(
                    usage.total_tokens,
                    usage.input_tokens,
                    usage.output_tokens,
                )
                .thought_tokens(usage.thought_tokens)
                .cached_read_tokens(usage.cached_read_tokens)
                .cached_write_tokens(usage.cached_write_tokens),
            );
        }
        response
    }
    fn request_permission(
        request: PromptRequest,
        responder: Responder<PromptResponse>,
        connection: &ConnectionTo<Client>,
    ) -> Result<(), agent_client_protocol::Error> {
        let permission = permission_request(request.session_id);
        connection
            .send_request(permission)
            .on_receiving_result(async move |result| {
                let response = result?;
                responder.respond(PromptResponse::new(permission_stop_reason(
                    &response.outcome,
                )))
            })
    }

    async fn replay_prompt(
        self: &Arc<Self>,
        request: PromptRequest,
        responder: Responder<PromptResponse>,
        connection: ConnectionTo<Client>,
    ) -> Result<(), agent_client_protocol::Error> {
        if request.session_id.0.as_ref() != self.fixture.manifest.session.session_id {
            return responder.respond(PromptResponse::new(StopReason::Refusal));
        }

        let replay_timing = self.fixture.manifest.prompt.replay_timing;
        let mut emitted = 0usize;

        for event in &self.fixture.events {
            if event.kind != "session/update" {
                continue;
            }

            if let ScenarioBehavior::DieMidStream { after_events } = self.behavior
                && emitted >= after_events
            {
                std::process::exit(1);
            }

            if let ScenarioBehavior::Malformed { after_events } = self.behavior
                && emitted >= after_events
            {
                use std::io::Write;
                let mut stdout = std::io::stdout();
                let _ = stdout.write_all(b"{ this is not valid json\n");
                let _ = stdout.flush();
                std::process::exit(0);
            }

            if let Some(notification_value) = &event.notification {
                let notification = parse_notification(notification_value)?;
                connection.send_notification(notification)?;

                emitted += 1;

                if replay_timing && event.t_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(event.t_ms)).await;
                }
            }
        }

        if matches!(self.behavior, ScenarioBehavior::CancelHonored) {
            while !self.cancelled.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        if matches!(self.behavior, ScenarioBehavior::IgnoreCancel) {
            std::future::pending::<()>().await;
        }

        responder.respond(self.prompt_response())
    }

    fn stop_reason(&self) -> StopReason {
        if self.cancelled.load(Ordering::SeqCst)
            && matches!(self.behavior, ScenarioBehavior::CancelHonored)
        {
            StopReason::Cancelled
        } else {
            map_stop_reason(self.fixture.manifest.prompt.stop_reason)
        }
    }
}

fn session_unavailable() -> agent_client_protocol::Error {
    agent_client_protocol::Error::invalid_params().data("fixture session unavailable")
}

fn permission_request(
    session_id: impl Into<agent_client_protocol::schema::v1::SessionId>,
) -> RequestPermissionRequest {
    RequestPermissionRequest::new(
        session_id,
        ToolCallUpdate::new(
            ToolCallId::new("fake-write"),
            ToolCallUpdateFields::default().title("Write fixture output"),
        ),
        vec![PermissionOption::new(
            PermissionOptionId::new("allow-once"),
            "Allow once",
            PermissionOptionKind::AllowOnce,
        )],
    )
}

fn permission_stop_reason(outcome: &RequestPermissionOutcome) -> StopReason {
    if matches!(outcome, RequestPermissionOutcome::Cancelled) {
        StopReason::EndTurn
    } else {
        StopReason::Refusal
    }
}

fn parse_notification(
    value: &serde_json::Value,
) -> Result<SessionNotification, agent_client_protocol::Error> {
    serde_json::from_value(value.clone())
        .map_err(|error| agent_client_protocol::Error::internal_error().data(error.to_string()))
}

fn map_stop_reason(reason: SubStopReason) -> StopReason {
    match reason {
        SubStopReason::MaxTokens => StopReason::MaxTokens,
        SubStopReason::MaxTurnRequests => StopReason::MaxTurnRequests,
        SubStopReason::Refusal => StopReason::Refusal,
        SubStopReason::Cancelled => StopReason::Cancelled,
        _ => StopReason::EndTurn,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, atomic::Ordering};

    use agent_client_protocol::schema::ProtocolVersion;
    use agent_client_protocol::schema::v1::{
        CancelNotification, InitializeRequest, NewSessionRequest, PermissionOptionId,
        RequestPermissionOutcome, SelectedPermissionOutcome, SessionId,
    };

    use super::*;
    use crate::fixture::LoadedFixture;

    fn minimal_fixture() -> LoadedFixture {
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/fixtures/minimal");
        LoadedFixture::load(dir).unwrap_or_else(|error| panic!("fixture: {error}"))
    }

    fn codex_fixture() -> LoadedFixture {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sub-harness-fake/fixtures/codex-hello");
        LoadedFixture::load(dir).unwrap_or_else(|error| panic!("fixture: {error}"))
    }

    fn shared_state(behavior: ScenarioBehavior) -> Arc<SharedState> {
        Arc::new(SharedState {
            behavior,
            fixture: minimal_fixture(),
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }

    #[test]
    fn map_stop_reason_covers_all_variants() {
        assert_eq!(map_stop_reason(SubStopReason::EndTurn), StopReason::EndTurn);
        assert_eq!(
            map_stop_reason(SubStopReason::MaxTokens),
            StopReason::MaxTokens
        );
        assert_eq!(
            map_stop_reason(SubStopReason::MaxTurnRequests),
            StopReason::MaxTurnRequests
        );
        assert_eq!(map_stop_reason(SubStopReason::Refusal), StopReason::Refusal);
        assert_eq!(
            map_stop_reason(SubStopReason::Cancelled),
            StopReason::Cancelled
        );
    }

    #[test]
    fn initialize_response_uses_fixture_agent_info() {
        let state = shared_state(ScenarioBehavior::Replay);
        let request = InitializeRequest::new(ProtocolVersion::V1);
        let response = state.initialize_response(&request);
        assert_eq!(
            response.agent_info.as_ref().map(|info| info.name.as_str()),
            Some(state.fixture.manifest.agent.name.as_str())
        );
    }

    #[test]
    fn new_session_response_uses_fixture_session_id() {
        let state = shared_state(ScenarioBehavior::Replay);
        let request = NewSessionRequest::new("/tmp");
        let response = state.new_session_response(request);
        assert_eq!(
            response.session_id.0.as_ref(),
            state.fixture.manifest.session.session_id
        );
    }

    #[test]
    fn resume_accepts_only_the_recorded_available_session() {
        let state = shared_state(ScenarioBehavior::Replay);
        let session_id = state.fixture.manifest.session.session_id.clone();
        let accepted = ResumeSessionRequest::new(session_id, "/tmp");
        assert!(state.resume_session(&accepted).is_ok());

        let missing = ResumeSessionRequest::new("missing-session", "/tmp");
        assert!(state.resume_session(&missing).is_err());

        let refused = shared_state(ScenarioBehavior::ResumeRefused);
        let recorded =
            ResumeSessionRequest::new(refused.fixture.manifest.session.session_id.clone(), "/tmp");
        assert!(refused.resume_session(&recorded).is_err());
    }

    #[test]
    fn load_returns_the_recorded_replay_updates_or_refuses() {
        let state = shared_state(ScenarioBehavior::Replay);
        let request =
            LoadSessionRequest::new(state.fixture.manifest.session.session_id.clone(), "/tmp");
        let updates = state
            .load_session(&request)
            .unwrap_or_else(|error| panic!("load: {error}"));
        let expected = state
            .fixture
            .events
            .iter()
            .filter(|event| event.kind == "session/update" && event.notification.is_some())
            .count();
        assert_eq!(updates.len(), expected);
        assert!(!updates.is_empty());

        let missing = LoadSessionRequest::new("missing-session", "/tmp");
        assert!(state.load_session(&missing).is_err());
    }

    #[test]
    fn prompt_response_preserves_fixture_usage_support() {
        let minimal = shared_state(ScenarioBehavior::Replay);
        assert!(minimal.prompt_response().usage.is_none());

        let codex = SharedState {
            behavior: ScenarioBehavior::Replay,
            fixture: codex_fixture(),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let usage = codex
            .prompt_response()
            .usage
            .unwrap_or_else(|| panic!("codex usage"));
        assert_eq!(usage.total_tokens, 16_749);
        assert_eq!(usage.input_tokens, 1_410);
        assert_eq!(usage.output_tokens, 235);
    }

    #[test]
    fn cancel_honored_stop_reason_when_flag_set() {
        let state = shared_state(ScenarioBehavior::CancelHonored);
        state.cancelled.store(true, Ordering::SeqCst);
        assert_eq!(state.stop_reason(), StopReason::Cancelled);
    }

    #[test]
    fn replay_uses_fixture_stop_reason_without_honored_cancel() {
        let state = shared_state(ScenarioBehavior::Replay);
        assert_eq!(state.stop_reason(), StopReason::EndTurn);
    }

    #[test]
    fn cancel_only_matches_fixture_session() {
        let state = shared_state(ScenarioBehavior::CancelHonored);
        state.record_cancel(&CancelNotification::new(SessionId::new("different")));
        assert!(!state.cancelled.load(Ordering::SeqCst));
        state.record_cancel(&CancelNotification::new(SessionId::new(
            state.fixture.manifest.session.session_id.clone(),
        )));
        assert!(state.cancelled.load(Ordering::SeqCst));
    }

    #[test]
    fn permission_request_has_allow_option_and_tool_title() {
        let request = permission_request(SessionId::new("session"));
        assert_eq!(request.session_id.0.as_ref(), "session");
        assert_eq!(
            request.tool_call.fields.title.as_deref(),
            Some("Write fixture output")
        );
        assert_eq!(request.options[0].option_id.0.as_ref(), "allow-once");
    }

    #[test]
    fn permission_outcome_controls_prompt_stop_reason() {
        assert_eq!(
            permission_stop_reason(&RequestPermissionOutcome::Cancelled),
            StopReason::EndTurn
        );
        let selected = RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
            PermissionOptionId::new("allow-once"),
        ));
        assert_eq!(permission_stop_reason(&selected), StopReason::Refusal);
    }

    #[test]
    fn fixture_notification_parser_accepts_valid_and_rejects_invalid() {
        let fixture = minimal_fixture();
        let value = fixture.events[0]
            .notification
            .as_ref()
            .unwrap_or_else(|| panic!("notification"));
        assert!(parse_notification(value).is_ok());
        assert!(parse_notification(&serde_json::json!({"invalid": true})).is_err());
    }
}
