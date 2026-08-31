//! Fake ACP agent process that replays fixtures over stdio.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    AgentCapabilities, InitializeRequest, InitializeResponse, NewSessionRequest,
    NewSessionResponse, PromptRequest, PromptResponse, SessionNotification, StopReason,
};
use agent_client_protocol::{Agent, Client, ConnectionTo, Responder, Stdio};

use super::fixture::{AgentInfo, LoadedFixture};
use super::scenario::{Scenario, ScenarioBehavior};
use crate::acp::error::AcpError;
use crate::acp::stop_reason::StopReason as SubStopReason;

/// Run the fake harness agent on stdio using the given scenario and fixture roots.
///
/// # Errors
///
/// Returns [`AcpError`] when fixtures cannot be loaded or the agent connection fails.
pub async fn run_stdio(
    scenarios_root: &Path,
    fixtures_root: &Path,
    scenario_name: &str,
) -> Result<(), AcpError> {
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
                        ScenarioBehavior::CancelHonored
                        | ScenarioBehavior::Replay
                        | ScenarioBehavior::IgnoreCancel
                        | ScenarioBehavior::DieMidStream { .. }
                        | ScenarioBehavior::Malformed { .. } => {
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
                    if notification.session_id.0.as_ref()
                        == state.fixture.manifest.session.session_id
                    {
                        state.cancelled.store(true, Ordering::SeqCst);
                    }
                    Ok(())
                }
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_to(Stdio::new())
        .await
        .map_err(|error| AcpError::protocol(&error))
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
                let notification: SessionNotification =
                    serde_json::from_value(notification_value.clone()).map_err(|error| {
                        agent_client_protocol::Error::internal_error().data(error.to_string())
                    })?;
                connection.send_notification(notification)?;

                emitted += 1;

                if replay_timing && event.t_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(event.t_ms)).await;
                }
            }
        }

        let stop_reason = if self.cancelled.load(Ordering::SeqCst)
            && matches!(self.behavior, ScenarioBehavior::CancelHonored)
        {
            StopReason::Cancelled
        } else {
            map_stop_reason(self.fixture.manifest.prompt.stop_reason)
        };
        responder.respond(PromptResponse::new(stop_reason))
    }
}

fn map_stop_reason(reason: SubStopReason) -> StopReason {
    match reason {
        SubStopReason::EndTurn => StopReason::EndTurn,
        SubStopReason::MaxTokens => StopReason::MaxTokens,
        SubStopReason::MaxTurnRequests => StopReason::MaxTurnRequests,
        SubStopReason::Refusal => StopReason::Refusal,
        SubStopReason::Cancelled => StopReason::Cancelled,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, atomic::Ordering};

    use agent_client_protocol::schema::ProtocolVersion;
    use agent_client_protocol::schema::v1::{InitializeRequest, NewSessionRequest};

    use super::*;
    use crate::acp::replay::fixture::LoadedFixture;

    fn minimal_fixture() -> LoadedFixture {
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/fixtures/minimal");
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
    fn cancel_honored_stop_reason_when_flag_set() {
        let state = shared_state(ScenarioBehavior::CancelHonored);
        state.cancelled.store(true, Ordering::SeqCst);
        let stop = if state.cancelled.load(Ordering::SeqCst)
            && matches!(state.behavior, ScenarioBehavior::CancelHonored)
        {
            StopReason::Cancelled
        } else {
            map_stop_reason(state.fixture.manifest.prompt.stop_reason)
        };
        assert_eq!(stop, StopReason::Cancelled);
    }
}
