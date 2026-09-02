use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::acp::{CancellationOptions, SessionObserver, StreamUpdateKind, UpdateObserver};

use super::result::looks_like_subagent;
use super::state::{TaskPaths, read_json, read_task_usage, write_json};
use super::supervisor::CANCEL_GRACE_PERIOD;
use super::{
    ActivityKind, DelegationError, ExecutionAttempt, TaskEvent, TaskEventKind, TaskHandle,
    UsageCost,
};

pub(super) fn cancellation_options(paths: &TaskPaths) -> CancellationOptions {
    CancellationOptions {
        request_path: paths.cancel_request.clone(),
        grace_period: CANCEL_GRACE_PERIOD,
    }
}

pub(super) fn session_observer(
    paths: &TaskPaths,
    handle: &TaskHandle,
    running: ExecutionAttempt,
    resumed: bool,
) -> SessionObserver {
    let state_path = paths.state.clone();
    let events = paths.events.clone();
    let handle = handle.clone();
    Arc::new(move |session_id| {
        let mut attempt = running.clone();
        attempt.harness_session_id = Some(session_id.to_owned());
        let _ = write_json(&state_path, &attempt);
        if resumed {
            let _ = append_event(
                &events,
                &handle,
                attempt.number,
                TaskEventKind::AttemptResumed,
            );
        }
    })
}

pub(super) fn update_observer(
    paths: &TaskPaths,
    handle: &TaskHandle,
    running: ExecutionAttempt,
) -> UpdateObserver {
    let observer_state = Arc::new(Mutex::new(ObserverState {
        events: paths.events.clone(),
        state: paths.state.clone(),
        task_root: paths.task_root.clone(),
        handle: handle.clone(),
        attempt: running,
        last_activity: None,
    }));
    Arc::new(move |update| {
        if let Ok(mut state) = observer_state.lock() {
            let subagent_observed = looks_like_subagent(&update);
            if let Some(cost) = &update.cost {
                if let Ok(current) = read_json::<ExecutionAttempt>(&state.state) {
                    state.attempt.harness_session_id = current.harness_session_id;
                }
                state.attempt.usage.cost = Some(UsageCost {
                    amount: cost.amount,
                    currency: cost.currency.clone(),
                });
                let _ = write_json(&state.state, &state.attempt);
                let usage = state.attempt.usage.clone();
                let task_usage =
                    read_task_usage(&state.task_root).unwrap_or_else(|_| usage.clone());
                let _ = append_event(
                    &state.events,
                    &state.handle,
                    state.attempt.number,
                    TaskEventKind::UsageAccumulated {
                        attempt_usage: Box::new(usage),
                        task_usage: Box::new(task_usage),
                    },
                );
            }
            if let Some(activity) = activity_kind(update.kind, subagent_observed)
                && state.last_activity != Some(activity)
            {
                state.last_activity = Some(activity);
                let _ = append_event(
                    &state.events,
                    &state.handle,
                    state.attempt.number,
                    TaskEventKind::Activity { activity },
                );
            }
        }
    })
}

struct ObserverState {
    events: PathBuf,
    state: PathBuf,
    task_root: PathBuf,
    handle: TaskHandle,
    attempt: ExecutionAttempt,
    last_activity: Option<ActivityKind>,
}

pub(super) fn activity_kind(
    kind: StreamUpdateKind,
    subagent_observed: bool,
) -> Option<ActivityKind> {
    if subagent_observed {
        return Some(ActivityKind::SubagentObserved);
    }
    match kind {
        StreamUpdateKind::AgentMessageChunk => Some(ActivityKind::Message),
        StreamUpdateKind::AgentThoughtChunk => Some(ActivityKind::Thought),
        StreamUpdateKind::ToolCall => Some(ActivityKind::ToolCall),
        StreamUpdateKind::ToolCallUpdate => Some(ActivityKind::ToolCallUpdate),
        StreamUpdateKind::SessionInfoUpdate => Some(ActivityKind::SessionInfo),
        StreamUpdateKind::AvailableCommandsUpdate => Some(ActivityKind::AvailableCommands),
        StreamUpdateKind::Plan => Some(ActivityKind::Plan),
        StreamUpdateKind::PermissionDenied => Some(ActivityKind::PermissionDenied),
        StreamUpdateKind::Other => Some(ActivityKind::Other),
        StreamUpdateKind::UsageUpdate => None,
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub(super) fn append_event(
    path: &Path,
    handle: &TaskHandle,
    attempt: u32,
    kind: TaskEventKind,
) -> Result<(), DelegationError> {
    let event = TaskEvent {
        timestamp_unix_ms: now_ms(),
        task_id: handle.id.clone(),
        attempt,
        kind,
    };
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, &event)?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

pub(super) fn read_events(path: &Path) -> Result<Vec<TaskEvent>, DelegationError> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    let complete = text.ends_with('\n');
    let mut events = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(line) {
            Ok(event) => events.push(event),
            Err(_) if !complete && index + 1 == text.lines().count() => break,
            Err(error) => return Err(DelegationError::Json(error)),
        }
    }
    Ok(events)
}
