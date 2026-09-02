use std::path::Path;
use std::process::Command;

use crate::acp::{AcpClient, AcpClientConfig, PromptOptions, SessionStart, TurnUsage};

use super::events::{append_event, cancellation_options, session_observer, update_observer};
use super::liveness::process_start_time;
use super::result::{base_artifacts, derive_task_result};
use super::state::{TaskPaths, read_json, read_task_usage, validate_handle, write_json};
use super::{
    DelegationError, ExecutionAttempt, ResumeFailureReason, ResumeMechanism, SupervisorRequest,
    TaskEventKind, TaskHandle, TaskResult, TaskStatus, UsageTotals,
};

#[cfg(test)]
pub(super) const CANCEL_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_secs(1);
#[cfg(not(test))]
pub(super) const CANCEL_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_secs(5);

pub(super) fn supervisor_command(executable: &Path) -> Command {
    #[cfg(target_os = "linux")]
    {
        let mut command = Command::new("setsid");
        command.arg("-f").arg(executable);
        command
    }
    #[cfg(not(target_os = "linux"))]
    {
        Command::new(executable)
    }
}

/// Run one numbered attempt supervisor for a previously launched handle.
///
/// # Errors
///
/// Returns an error when persisted state cannot be read or written.
pub async fn run_supervisor(
    state_dir: &Path,
    handle: &TaskHandle,
    number: u32,
) -> Result<(), DelegationError> {
    validate_handle(handle)?;
    let paths = TaskPaths::for_attempt(state_dir, handle, number);
    let request: SupervisorRequest = read_json(&paths.request)?;
    let running = start_attempt(&paths, handle, number)?;
    let is_resume = request.resume_session_id.is_some();
    let harness = request.params.harness;
    let cwd = request.params.cwd.clone();
    if number > 1 && request.resume_session_id.is_none() {
        let completion = missing_session_completion(&paths, handle, number)?;
        return finish_attempt(&paths, handle, number, completion);
    }
    let outcome = execute_prompt(&paths, handle, running, request).await;
    record_resume_failure(&paths, handle, number, is_resume, &outcome)?;
    let cancellation_honored = outcome
        .as_ref()
        .ok()
        .and_then(|(_, result)| result.cancellation_honored);
    let (result, tokens) = derive_task_result(outcome, &paths, harness, &cwd);
    finish_attempt(
        &paths,
        handle,
        number,
        AttemptCompletion {
            result,
            tokens,
            cancellation_honored,
        },
    )
}

fn start_attempt(
    paths: &TaskPaths,
    handle: &TaskHandle,
    number: u32,
) -> Result<ExecutionAttempt, DelegationError> {
    let running = ExecutionAttempt {
        number,
        status: TaskStatus::Running,
        supervisor_pid: Some(std::process::id()),
        supervisor_start_time: process_start_time(std::process::id()),
        harness_session_id: None,
        usage: UsageTotals::default(),
    };
    write_json(&paths.state, &running)?;
    append_event(&paths.events, handle, number, TaskEventKind::AttemptStarted)?;
    Ok(running)
}

fn missing_session_completion(
    paths: &TaskPaths,
    handle: &TaskHandle,
    number: u32,
) -> Result<AttemptCompletion, DelegationError> {
    append_event(
        &paths.events,
        handle,
        number,
        TaskEventKind::AttemptResumeFailed {
            reason: ResumeFailureReason::SessionRecordMissing,
        },
    )?;
    Ok(AttemptCompletion {
        result: TaskResult {
            status: TaskStatus::Failed,
            summary: "recovery failed: orphaned attempt has no recorded harness session id"
                .to_owned(),
            changed_files: Vec::new(),
            artifacts: base_artifacts(paths),
            harness_session_id: None,
        },
        tokens: None,
        cancellation_honored: None,
    })
}

async fn execute_prompt(
    paths: &TaskPaths,
    handle: &TaskHandle,
    running: ExecutionAttempt,
    request: SupervisorRequest,
) -> Result<(crate::acp::SessionHandle, crate::acp::PromptResult), crate::acp::AcpError> {
    let is_resume = request.resume_session_id.is_some();
    let observer = update_observer(paths, handle, running.clone());
    let session_observer = session_observer(paths, handle, running, is_resume);
    let prompt = prompt_text(&request, is_resume);
    let options = PromptOptions {
        update_observer: Some(observer),
        session_observer: Some(session_observer),
        ..prompt_options(&request, paths)
    };
    AcpClient::new(request.adapter.bridge, AcpClientConfig::default())
        .prompt_turn(&request.params.cwd, &prompt, options)
        .await
}

fn prompt_text(request: &SupervisorRequest, is_resume: bool) -> String {
    if is_resume {
        format!(
            "Continue the delegated task from the interrupted attempt. Do not restart it from scratch.\n\n{}",
            request.adapter.delegation_guard
        )
    } else {
        format!(
            "{}\n\n{}",
            request.params.prompt, request.adapter.delegation_guard
        )
    }
}

fn prompt_options(request: &SupervisorRequest, paths: &TaskPaths) -> PromptOptions {
    let session_start =
        request
            .resume_session_id
            .as_ref()
            .map_or(SessionStart::New, |session_id| {
                match request.adapter.resume_mechanism {
                    ResumeMechanism::Resume => SessionStart::Resume(session_id.clone()),
                    ResumeMechanism::Load => SessionStart::Load(session_id.clone()),
                }
            });
    PromptOptions {
        permission_mode: Some(request.params.permission_mode.clone()),
        model: request.params.model.clone(),
        session_meta: Some(request.adapter.session_meta.clone()),
        session_start,
        cancellation: Some(cancellation_options(paths)),
        ..PromptOptions::default()
    }
}

fn record_resume_failure(
    paths: &TaskPaths,
    handle: &TaskHandle,
    number: u32,
    is_resume: bool,
    outcome: &Result<(crate::acp::SessionHandle, crate::acp::PromptResult), crate::acp::AcpError>,
) -> Result<(), DelegationError> {
    if is_resume && outcome.is_err() {
        append_event(
            &paths.events,
            handle,
            number,
            TaskEventKind::AttemptResumeFailed {
                reason: ResumeFailureReason::HarnessRefused,
            },
        )?;
    }
    Ok(())
}

struct AttemptCompletion {
    result: TaskResult,
    tokens: Option<TurnUsage>,
    cancellation_honored: Option<bool>,
}

fn finish_attempt(
    paths: &TaskPaths,
    handle: &TaskHandle,
    number: u32,
    completion: AttemptCompletion,
) -> Result<(), DelegationError> {
    let AttemptCompletion {
        result,
        tokens,
        cancellation_honored,
    } = completion;
    let current: ExecutionAttempt = read_json(&paths.state)?;
    let mut usage = current.usage.clone();
    usage.tokens = tokens;
    if usage.tokens.is_some() {
        write_json(
            &paths.state,
            &ExecutionAttempt {
                usage: usage.clone(),
                ..current.clone()
            },
        )?;
        let task_usage = read_task_usage(&paths.task_root)?;
        append_event(
            &paths.events,
            handle,
            number,
            TaskEventKind::UsageAccumulated {
                attempt_usage: Box::new(usage.clone()),
                task_usage: Box::new(task_usage),
            },
        )?;
    }
    write_json(&paths.result, &result)?;
    write_json(
        &paths.state,
        &ExecutionAttempt {
            number,
            status: result.status,
            supervisor_pid: Some(std::process::id()),
            supervisor_start_time: current.supervisor_start_time,
            harness_session_id: result
                .harness_session_id
                .clone()
                .or(current.harness_session_id),
            usage,
        },
    )?;
    if result.status == TaskStatus::Cancelled {
        append_event(
            &paths.events,
            handle,
            number,
            TaskEventKind::AttemptCancelled {
                harness_honored: cancellation_honored.unwrap_or(true),
            },
        )?;
    }
    append_event(
        &paths.events,
        handle,
        number,
        TaskEventKind::AttemptFinished {
            status: result.status,
        },
    )?;
    Ok(())
}
