//! Durable delegated-task launch, supervision, and wait semantics.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::acp::{
    AcpClient, AcpClientConfig, HarnessLaunch, PromptOptions, SessionObserver, SessionStart,
    StopReason, StreamUpdate, StreamUpdateKind, TurnUsage, UpdateObserver,
};

static HANDLE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Supported child harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Harness {
    /// Claude Code.
    Claude,
    /// `OpenAI Codex`.
    Codex,
}

impl Harness {
    fn usage_support(self) -> UsageSupport {
        match self {
            Self::Claude => UsageSupport::claude(),
            Self::Codex => UsageSupport::codex(),
        }
    }
}

/// Whether a harness is known to report each usage measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageSupport {
    /// The harness reports cumulative monetary cost.
    pub cost: bool,
    /// The harness reports per-turn token usage.
    pub tokens: bool,
}

impl UsageSupport {
    /// Claude bridge support verified by the ACP boundary proof.
    #[must_use]
    pub const fn claude() -> Self {
        Self {
            cost: true,
            tokens: true,
        }
    }

    /// Codex bridge support verified by the ACP boundary proof.
    #[must_use]
    pub const fn codex() -> Self {
        Self {
            cost: false,
            tokens: true,
        }
    }
}

/// Monetary cost reported by a harness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageCost {
    /// Numeric amount in the reported currency.
    pub amount: f64,
    /// ISO 4217 currency code.
    pub currency: String,
}

/// Usage accumulated by `sub`; missing measurements are never replaced by zero.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageTotals {
    /// Latest cumulative cost, or absent when not reported.
    pub cost: Option<UsageCost>,
    /// Sum of per-turn token usage, or absent when not reported.
    pub tokens: Option<TurnUsage>,
}

/// Opaque identifier returned by launch and accepted by every task control.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskHandle {
    /// Stable `sub` task identifier.
    pub id: String,
}

/// Parameters for one bounded delegated task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchParams {
    /// Child harness.
    pub harness: Harness,
    /// Bounded task prompt.
    pub prompt: String,
    /// Existing directory in which the child runs, unchanged by `sub`.
    pub cwd: PathBuf,
    /// User-owned harness binary path.
    pub harness_binary: PathBuf,
    /// Optional harness-native model identifier.
    pub model: Option<String>,
    /// Optional harness-native permission mode identifier.
    pub permission_mode: String,
}

/// One semantic delegated task and its initial execution attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegatedTask {
    /// Stable task identity used by controls.
    pub handle: TaskHandle,
    /// Inputs supplied by the manager.
    pub params: LaunchParams,
    /// The task's initial execution attempt; recovery attempts live in numbered state directories.
    pub attempt: ExecutionAttempt,
}

/// One process invocation of a harness under a delegated task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionAttempt {
    /// Sequential attempt number within the task.
    pub number: u32,
    /// Latest durable lifecycle status.
    pub status: TaskStatus,
    /// Supervisor process identifier when known.
    pub supervisor_pid: Option<u32>,
    /// Operating-system process start token used to reject PID reuse.
    #[serde(default)]
    pub supervisor_start_time: Option<u64>,
    /// Vendor-owned session identifier once session creation succeeds.
    pub harness_session_id: Option<String>,
    /// Usage accumulated for this attempt.
    #[serde(default)]
    pub usage: UsageTotals,
}

/// Adapter-prepared launch data consumed by the shared ACP client layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterLaunch {
    /// Pinned bridge command and side-channel environment.
    pub bridge: HarnessLaunch,
    /// Bridge-specific `session/new` metadata.
    pub session_meta: serde_json::Value,
    /// Prompt guard enforcing one level of delegation.
    pub delegation_guard: String,
    /// ACP mechanism the bridge implements for cross-process session recovery.
    pub resume_mechanism: ResumeMechanism,
}

/// ACP v1 mechanism used by a harness bridge to reopen a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeMechanism {
    /// `session/resume`, without replaying the transcript.
    Resume,
    /// `session/load`, whose bridge may replay the transcript as updates.
    Load,
}

/// Lifecycle status derived and persisted by `sub`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Supervisor has not started the ACP connection yet.
    Queued,
    /// The latest execution attempt is active.
    Running,
    /// The attempt was running, but its recorded supervisor process is gone.
    Orphaned,
    /// The child ended its turn normally.
    Succeeded,
    /// The child or bridge failed, refused, or exhausted a limit.
    Failed,
    /// The child reported cancellation.
    Cancelled,
}

/// Kind of artifact retained by reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// `sub`'s normalized event log.
    EventLog,
    /// Harness-owned native session record; never copied by `sub`.
    NativeSession,
    /// Supervisor diagnostic output.
    SupervisorLog,
}

/// Reference to durable evidence without embedding its content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReference {
    /// Artifact category.
    pub kind: ArtifactKind,
    /// Local path or stable native-session locator.
    pub location: String,
}

/// Bounded handoff derived from the child's stream and stop reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskResult {
    /// Derived terminal status.
    pub status: TaskStatus,
    /// Concatenated final assistant message, or a failure summary.
    pub summary: String,
    /// Unique paths attached to edit/delete/move stream events.
    pub changed_files: Vec<PathBuf>,
    /// Evidence retained by reference.
    pub artifacts: Vec<ArtifactReference>,
    /// Vendor-owned resumable conversation identity, when session creation succeeded.
    pub harness_session_id: Option<String>,
}

/// Outcome of a bounded wait call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WaitOutcome {
    /// The task did not reach a terminal state before the timeout.
    Running {
        /// Latest persisted lifecycle status.
        status: TaskStatus,
    },
    /// The latest attempt's recorded supervisor is no longer alive.
    Orphaned {
        /// Explicit orphaned lifecycle status.
        status: TaskStatus,
    },
    /// The task has a durable result.
    Complete {
        /// Derived result handoff.
        result: TaskResult,
    },
}

/// A small normalized activity category that does not copy transcript content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    /// Assistant output arrived.
    Message,
    /// Agent reasoning arrived.
    Thought,
    /// A tool call started.
    ToolCall,
    /// A tool call changed state.
    ToolCallUpdate,
    /// A plan changed.
    Plan,
    /// Session metadata changed.
    SessionInfo,
    /// Available commands changed.
    AvailableCommands,
    /// A permission request was denied by `sub`.
    PermissionDenied,
    /// A tool title indicated forbidden nested delegation.
    SubagentObserved,
    /// An unrecognized ACP update arrived.
    Other,
}

/// Stable event vocabulary owned by `sub`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEventKind {
    /// A task was linked to its initial attempt.
    TaskCreated,
    /// A new attempt started.
    AttemptStarted,
    /// A running attempt's recorded supervisor was no longer alive.
    AttemptOrphaned,
    /// A harness session was resumed in a replacement attempt.
    AttemptResumed,
    /// Recovery could not resume the recorded harness session.
    AttemptResumeFailed {
        /// Stable failure category; diagnostics remain in the supervisor log and result.
        reason: ResumeFailureReason,
    },
    /// An attempt was cancelled.
    AttemptCancelled,
    /// An attempt reached a terminal state.
    AttemptFinished {
        /// Terminal attempt status.
        status: TaskStatus,
    },
    /// Normalized activity arrived without transcript content.
    Activity {
        /// Normalized activity category.
        activity: ActivityKind,
    },
    /// Cost or tokens changed for the attempt and task.
    UsageAccumulated {
        /// Accumulated usage for this attempt.
        attempt_usage: Box<UsageTotals>,
        /// Accumulated usage across the task's attempts.
        task_usage: Box<UsageTotals>,
    },
}

/// Why a recovery attempt could not resume its predecessor's harness session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeFailureReason {
    /// The orphaned attempt had not durably recorded a harness session identity.
    SessionRecordMissing,
    /// The fresh bridge process refused or failed to reopen the recorded session.
    HarnessRefused,
}

/// Immediate response from explicit recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverOutcome {
    /// Stable delegated-task handle.
    pub handle: TaskHandle,
    /// Newly created sequential attempt number.
    pub attempt: u32,
}

/// One append-only event record written by the supervisor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEvent {
    /// Unix time in milliseconds.
    pub timestamp_unix_ms: u128,
    /// Task linked to this attempt.
    pub task_id: String,
    /// Attempt number within the task.
    pub attempt: u32,
    /// Typed normalized event payload.
    #[serde(flatten)]
    pub kind: TaskEventKind,
}

/// Compact task state returned by list and embedded by inspect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskOverview {
    /// Stable task handle.
    pub handle: TaskHandle,
    /// Child harness.
    pub harness: Harness,
    /// Latest task lifecycle state.
    pub status: TaskStatus,
    /// Known reporting support for the harness.
    pub usage_support: UsageSupport,
    /// Usage accumulated across attempts.
    pub usage: UsageTotals,
}

/// Response shape for listing delegated tasks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskList {
    /// Tasks ordered by opaque handle for deterministic output.
    pub tasks: Vec<TaskOverview>,
}

/// One attempt included in task inspection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptObservation {
    /// Attempt number.
    pub number: u32,
    /// Latest lifecycle status.
    pub status: TaskStatus,
    /// Vendor-owned session identity, when known.
    pub harness_session_id: Option<String>,
    /// Usage accumulated for this attempt.
    pub usage: UsageTotals,
}

/// Full read-only observation of one task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskInspection {
    /// Task-level status and accumulated usage.
    pub task: TaskOverview,
    /// Attempts contributing to the task totals.
    pub attempts: Vec<AttemptObservation>,
    /// Normalized append-only events across attempts.
    pub events: Vec<TaskEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SupervisorRequest {
    params: LaunchParams,
    adapter: AdapterLaunch,
    #[serde(default)]
    resume_session_id: Option<String>,
}

/// Delegation kernel bound to a private state directory and supervisor executable.
#[derive(Debug, Clone)]
pub struct Delegator {
    state_dir: PathBuf,
    supervisor_executable: PathBuf,
}

/// Delegation persistence or supervisor error.
#[derive(Debug, thiserror::Error)]
pub enum DelegationError {
    /// Filesystem operation failed.
    #[error("delegation state operation failed: {0}")]
    Io(#[from] io::Error),
    /// Persisted JSON was invalid.
    #[error("delegation state is invalid: {0}")]
    Json(#[from] serde_json::Error),
    /// Launch parameters were invalid.
    #[error("invalid launch parameters: {0}")]
    InvalidParams(String),
    /// The handle does not name a known task.
    #[error("unknown task handle: {0}")]
    UnknownHandle(String),
    /// The task's latest attempt is not orphaned and cannot be recovered.
    #[error("task is not orphaned: {0}")]
    NotOrphaned(String),
}

impl Delegator {
    /// Bind the kernel to a state directory and the executable providing supervision.
    #[must_use]
    pub fn new(state_dir: impl Into<PathBuf>, supervisor_executable: impl Into<PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
            supervisor_executable: supervisor_executable.into(),
        }
    }

    /// Persist one task and spawn its independent supervisor, returning immediately.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths, persistence failure, or supervisor spawn failure.
    pub fn launch(
        &self,
        params: LaunchParams,
        adapter: AdapterLaunch,
    ) -> Result<TaskHandle, DelegationError> {
        if !params.cwd.is_absolute() || !params.cwd.is_dir() {
            return Err(DelegationError::InvalidParams(
                "cwd must be an existing absolute directory".to_owned(),
            ));
        }
        if !params.harness_binary.is_absolute() || !params.harness_binary.is_file() {
            return Err(DelegationError::InvalidParams(
                "harness_binary must be an existing absolute file".to_owned(),
            ));
        }
        let handle = TaskHandle {
            id: new_handle(&params),
        };
        let paths = TaskPaths::new(&self.state_dir, &handle);
        fs::create_dir_all(&paths.attempt)?;
        let attempt = ExecutionAttempt {
            number: 1,
            status: TaskStatus::Queued,
            supervisor_pid: None,
            supervisor_start_time: None,
            harness_session_id: None,
            usage: UsageTotals::default(),
        };
        write_json(
            &paths.task,
            &DelegatedTask {
                handle: handle.clone(),
                params: params.clone(),
                attempt: attempt.clone(),
            },
        )?;
        write_json(
            &paths.request,
            &SupervisorRequest {
                params,
                adapter,
                resume_session_id: None,
            },
        )?;
        write_json(&paths.state, &attempt)?;
        append_event(&paths.events, &handle, 1, TaskEventKind::TaskCreated)?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.supervisor_log)?;
        let mut command = supervisor_command(&self.supervisor_executable);
        let child = command
            .args(["__supervise", &handle.id, "1", "--state-dir"])
            .arg(&self.state_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        write_json(
            &paths.state,
            &ExecutionAttempt {
                number: 1,
                status: TaskStatus::Queued,
                supervisor_pid: Some(child.id()),
                supervisor_start_time: process_start_time(child.id()),
                harness_session_id: None,
                usage: UsageTotals::default(),
            },
        )?;
        Ok(handle)
    }

    /// Start a new sequential attempt that resumes an orphaned attempt's harness session.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown task, a latest attempt that is not orphaned, persistence
    /// failure, or supervisor spawn failure.
    pub fn recover(&self, handle: &TaskHandle) -> Result<RecoverOutcome, DelegationError> {
        validate_handle(handle)?;
        let initial_paths = TaskPaths::new(&self.state_dir, handle);
        if !initial_paths.task.is_file() || !initial_paths.state.is_file() {
            return Err(DelegationError::UnknownHandle(handle.id.clone()));
        }
        let prior_number = latest_attempt_number(&self.state_dir, handle)?;
        let prior_paths = TaskPaths::for_attempt(&self.state_dir, handle, prior_number);
        let prior: ExecutionAttempt = read_json(&prior_paths.state)?;
        if effective_status(&prior) != TaskStatus::Orphaned {
            return Err(DelegationError::NotOrphaned(handle.id.clone()));
        }
        append_event(
            &prior_paths.events,
            handle,
            prior_number,
            TaskEventKind::AttemptOrphaned,
        )?;

        let number = prior_number + 1;
        let paths = TaskPaths::for_attempt(&self.state_dir, handle, number);
        fs::create_dir(&paths.attempt)?;
        let mut request: SupervisorRequest = read_json(&prior_paths.request)?;
        request
            .resume_session_id
            .clone_from(&prior.harness_session_id);
        write_json(&paths.request, &request)?;
        let queued = ExecutionAttempt {
            number,
            status: TaskStatus::Queued,
            supervisor_pid: None,
            supervisor_start_time: None,
            harness_session_id: prior.harness_session_id,
            usage: UsageTotals::default(),
        };
        write_json(&paths.state, &queued)?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.supervisor_log)?;
        let mut command = supervisor_command(&self.supervisor_executable);
        let child = command
            .args([
                "__supervise",
                &handle.id,
                &number.to_string(),
                "--state-dir",
            ])
            .arg(&self.state_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        write_json(
            &paths.state,
            &ExecutionAttempt {
                supervisor_pid: Some(child.id()),
                supervisor_start_time: process_start_time(child.id()),
                ..queued
            },
        )?;
        Ok(RecoverOutcome {
            handle: handle.clone(),
            attempt: number,
        })
    }

    /// Wait up to `timeout` for a durable result; a caller may repeat this with the same handle.
    ///
    /// # Errors
    ///
    /// Returns an error when the handle is unknown or persisted state cannot be read.
    pub async fn wait(
        &self,
        handle: &TaskHandle,
        timeout: Duration,
    ) -> Result<WaitOutcome, DelegationError> {
        validate_handle(handle)?;
        let initial_paths = TaskPaths::new(&self.state_dir, handle);
        if !initial_paths.state.is_file() {
            return Err(DelegationError::UnknownHandle(handle.id.clone()));
        }
        let deadline = Instant::now() + timeout;
        loop {
            let number = latest_attempt_number(&self.state_dir, handle)?;
            let paths = TaskPaths::for_attempt(&self.state_dir, handle, number);
            if paths.result.is_file() {
                return Ok(WaitOutcome::Complete {
                    result: read_json(&paths.result)?,
                });
            }
            let state: ExecutionAttempt = read_json(&paths.state)?;
            let status = effective_status(&state);
            if status == TaskStatus::Orphaned {
                return Ok(WaitOutcome::Orphaned { status });
            }
            if Instant::now() >= deadline {
                return Ok(WaitOutcome::Running { status });
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// List every delegated task readable from this state directory.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted task state cannot be read.
    pub fn list(&self) -> Result<TaskList, DelegationError> {
        let root = self.state_dir.join("tasks");
        if !root.is_dir() {
            return Ok(TaskList { tasks: Vec::new() });
        }
        let mut handles = fs::read_dir(root)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .map(|id| TaskHandle { id })
            .filter(|handle| validate_handle(handle).is_ok())
            .collect::<Vec<_>>();
        handles.sort_by(|left, right| left.id.cmp(&right.id));
        let tasks = handles
            .iter()
            .map(|handle| self.inspect(handle).map(|inspection| inspection.task))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TaskList { tasks })
    }

    /// Inspect one task without contacting its supervisor or harness.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown handle or unreadable persisted state.
    pub fn inspect(&self, handle: &TaskHandle) -> Result<TaskInspection, DelegationError> {
        validate_handle(handle)?;
        let paths = TaskPaths::new(&self.state_dir, handle);
        if !paths.task.is_file() || !paths.state.is_file() {
            return Err(DelegationError::UnknownHandle(handle.id.clone()));
        }
        let persisted: DelegatedTask = read_json(&paths.task)?;
        let (attempts, events, usage) = read_attempts(&self.state_dir, handle)?;
        let latest = attempts
            .last()
            .ok_or_else(|| DelegationError::UnknownHandle(handle.id.clone()))?;
        let overview = TaskOverview {
            handle: persisted.handle,
            harness: persisted.params.harness,
            status: latest.status,
            usage_support: persisted.params.harness.usage_support(),
            usage,
        };
        Ok(TaskInspection {
            task: overview,
            attempts,
            events,
        })
    }
}

fn supervisor_command(executable: &Path) -> Command {
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

    let observer = update_observer(&paths, handle, running.clone());
    let is_resume = request.resume_session_id.is_some();
    let session_observer = session_observer(&paths, handle, running.clone(), is_resume);
    let harness = request.params.harness;
    let cwd = request.params.cwd.clone();
    if number > 1 && request.resume_session_id.is_none() {
        append_event(
            &paths.events,
            handle,
            number,
            TaskEventKind::AttemptResumeFailed {
                reason: ResumeFailureReason::SessionRecordMissing,
            },
        )?;
        let result = TaskResult {
            status: TaskStatus::Failed,
            summary: "recovery failed: orphaned attempt has no recorded harness session id"
                .to_owned(),
            changed_files: Vec::new(),
            artifacts: base_artifacts(&paths),
            harness_session_id: None,
        };
        return finish_attempt(&paths, handle, number, &result, None);
    }
    let prompt = if is_resume {
        format!(
            "Continue the delegated task from the interrupted attempt. Do not restart it from scratch.\n\n{}",
            request.adapter.delegation_guard
        )
    } else {
        format!(
            "{}\n\n{}",
            request.params.prompt, request.adapter.delegation_guard
        )
    };
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
    let client = AcpClient::new(request.adapter.bridge, AcpClientConfig::default());
    let outcome = client
        .prompt_turn_observing_session(
            &request.params.cwd,
            &prompt,
            PromptOptions {
                permission_mode: Some(request.params.permission_mode),
                model: request.params.model,
                session_meta: Some(request.adapter.session_meta),
                session_start,
                ..PromptOptions::default()
            },
            Some(observer),
            Some(session_observer),
        )
        .await;

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

    let (result, tokens) = derive_task_result(outcome, &paths, harness, &cwd);
    finish_attempt(&paths, handle, number, &result, tokens)
}

fn session_observer(
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

fn update_observer(
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

fn derive_task_result(
    outcome: Result<(crate::acp::SessionHandle, crate::acp::PromptResult), crate::acp::AcpError>,
    paths: &TaskPaths,
    harness: Harness,
    cwd: &Path,
) -> (TaskResult, Option<TurnUsage>) {
    match outcome {
        Ok((session, prompt_result)) => (
            TaskResult {
                status: status_from_stop(prompt_result.stop_reason),
                summary: prompt_result.final_text.trim().to_owned(),
                changed_files: derive_changed_files(
                    &prompt_result.updates,
                    &prompt_result.final_text,
                    cwd,
                ),
                artifacts: artifacts(paths, harness, cwd, &session.session_id),
                harness_session_id: Some(session.session_id),
            },
            prompt_result.usage,
        ),
        Err(error) => (
            TaskResult {
                status: TaskStatus::Failed,
                summary: error.to_string(),
                changed_files: Vec::new(),
                artifacts: base_artifacts(paths),
                harness_session_id: None,
            },
            None,
        ),
    }
}

fn finish_attempt(
    paths: &TaskPaths,
    handle: &TaskHandle,
    number: u32,
    result: &TaskResult,
    tokens: Option<TurnUsage>,
) -> Result<(), DelegationError> {
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
            TaskEventKind::AttemptCancelled,
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

struct ObserverState {
    events: PathBuf,
    state: PathBuf,
    task_root: PathBuf,
    handle: TaskHandle,
    attempt: ExecutionAttempt,
    last_activity: Option<ActivityKind>,
}

fn activity_kind(kind: StreamUpdateKind, subagent_observed: bool) -> Option<ActivityKind> {
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

fn derive_changed_files(updates: &[StreamUpdate], final_text: &str, cwd: &Path) -> Vec<PathBuf> {
    let mut files = updates
        .iter()
        .flat_map(|update| update.changed_files.iter().cloned())
        .collect::<BTreeSet<_>>();
    for destination in markdown_destinations(final_text) {
        let path = PathBuf::from(destination);
        let absolute = if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        };
        if absolute.starts_with(cwd) && absolute.is_file() {
            files.insert(absolute);
        }
    }
    files.into_iter().collect()
}

fn looks_like_subagent(update: &StreamUpdate) -> bool {
    if !matches!(
        update.kind,
        crate::acp::StreamUpdateKind::ToolCall | crate::acp::StreamUpdateKind::ToolCallUpdate
    ) {
        return false;
    }
    update.text.as_deref().is_some_and(|title| {
        let normalized = title.to_ascii_lowercase();
        normalized == "agent"
            || normalized == "task"
            || normalized.contains("subagent")
            || normalized.contains("spawn_agent")
    })
}

fn markdown_destinations(text: &str) -> Vec<&str> {
    let mut destinations = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("](") {
        remaining = &remaining[start + 2..];
        let (candidate, rest) = if let Some(stripped) = remaining.strip_prefix('<') {
            let Some(end) = stripped.find(">)") else {
                break;
            };
            (&stripped[..end], &stripped[end + 2..])
        } else {
            let Some(end) = remaining.find(')') else {
                break;
            };
            (&remaining[..end], &remaining[end + 1..])
        };
        destinations.push(candidate);
        remaining = rest;
    }
    destinations
}

fn status_from_stop(reason: StopReason) -> TaskStatus {
    match reason {
        StopReason::EndTurn => TaskStatus::Succeeded,
        StopReason::Cancelled => TaskStatus::Cancelled,
        StopReason::MaxTokens | StopReason::MaxTurnRequests | StopReason::Refusal => {
            TaskStatus::Failed
        }
    }
}

fn effective_status(attempt: &ExecutionAttempt) -> TaskStatus {
    if attempt.status == TaskStatus::Running && !supervisor_is_alive(attempt) {
        TaskStatus::Orphaned
    } else {
        attempt.status
    }
}

#[cfg(target_os = "linux")]
fn process_start_time(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields = stat
        .rsplit_once(") ")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    fields.get(19)?.parse().ok()
}

#[cfg(not(target_os = "linux"))]
fn process_start_time(_pid: u32) -> Option<u64> {
    None
}

fn supervisor_is_alive(attempt: &ExecutionAttempt) -> bool {
    let Some(pid) = attempt.supervisor_pid else {
        return false;
    };
    #[cfg(target_os = "linux")]
    {
        let Some(expected) = attempt.supervisor_start_time else {
            return false;
        };
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        let Some((_, rest)) = stat.rsplit_once(") ") else {
            return false;
        };
        let fields = rest.split_whitespace().collect::<Vec<_>>();
        fields.first().is_some_and(|state| *state != "Z")
            && fields.get(19).and_then(|value| value.parse::<u64>().ok()) == Some(expected)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success())
    }
}

fn base_artifacts(paths: &TaskPaths) -> Vec<ArtifactReference> {
    vec![
        ArtifactReference {
            kind: ArtifactKind::EventLog,
            location: paths.events.display().to_string(),
        },
        ArtifactReference {
            kind: ArtifactKind::SupervisorLog,
            location: paths.supervisor_log.display().to_string(),
        },
    ]
}

fn artifacts(
    paths: &TaskPaths,
    harness: Harness,
    cwd: &Path,
    session_id: &str,
) -> Vec<ArtifactReference> {
    let mut values = base_artifacts(paths);
    values.push(ArtifactReference {
        kind: ArtifactKind::NativeSession,
        location: native_session_reference(harness, cwd, session_id),
    });
    values
}

fn native_session_reference(harness: Harness, cwd: &Path, session_id: &str) -> String {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return format!(
            "{}:{session_id}",
            match harness {
                Harness::Claude => "claude",
                Harness::Codex => "codex",
            }
        );
    };
    let root = match harness {
        Harness::Claude => home.join(".claude/projects"),
        Harness::Codex => home.join(".codex/sessions"),
    };
    find_session_record(&root, session_id).map_or_else(
        || {
            format!(
                "{}:{}:{}",
                match harness {
                    Harness::Claude => "claude",
                    Harness::Codex => "codex",
                },
                cwd.display(),
                session_id
            )
        },
        |path| path.display().to_string(),
    )
}

fn find_session_record(root: &Path, session_id: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_session_record(&path, session_id) {
                return Some(found);
            }
        } else if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains(session_id))
        {
            return Some(path);
        }
    }
    None
}

fn new_handle(params: &LaunchParams) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut digest = Sha256::new();
    digest.update(now.to_le_bytes());
    digest.update(std::process::id().to_le_bytes());
    digest.update(counter.to_le_bytes());
    digest.update(params.cwd.to_string_lossy().as_bytes());
    digest.update(params.prompt.as_bytes());
    format!("tsk_{}", &hex::encode(digest.finalize())[..24])
}

fn validate_handle(handle: &TaskHandle) -> Result<(), DelegationError> {
    let suffix = handle.id.strip_prefix("tsk_").ok_or_else(|| {
        DelegationError::InvalidParams("task handle must use the tsk_ prefix".to_owned())
    })?;
    if suffix.len() != 24
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DelegationError::InvalidParams(
            "task handle must contain 24 lowercase hexadecimal characters".to_owned(),
        ));
    }
    Ok(())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), DelegationError> {
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temp, path)?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, DelegationError> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn append_event(
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

fn read_events(path: &Path) -> Result<Vec<TaskEvent>, DelegationError> {
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

fn read_attempts(
    state_dir: &Path,
    handle: &TaskHandle,
) -> Result<(Vec<AttemptObservation>, Vec<TaskEvent>, UsageTotals), DelegationError> {
    let root = state_dir.join("tasks").join(&handle.id).join("attempts");
    let mut numbers = fs::read_dir(&root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
        .collect::<Vec<_>>();
    numbers.sort_unstable();
    let mut attempts = Vec::new();
    let mut events = Vec::new();
    let mut usage = UsageTotals::default();
    for number in numbers {
        let paths = TaskPaths::for_attempt(state_dir, handle, number);
        if !paths.state.is_file() {
            continue;
        }
        let attempt: ExecutionAttempt = read_json(&paths.state)?;
        add_usage(&mut usage, &attempt.usage);
        attempts.push(AttemptObservation {
            number: attempt.number,
            status: effective_status(&attempt),
            harness_session_id: attempt.harness_session_id,
            usage: attempt.usage,
        });
        events.extend(read_events(&paths.events)?);
    }
    Ok((attempts, events, usage))
}

fn latest_attempt_number(state_dir: &Path, handle: &TaskHandle) -> Result<u32, DelegationError> {
    let root = state_dir.join("tasks").join(&handle.id).join("attempts");
    fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
        .max()
        .ok_or_else(|| DelegationError::UnknownHandle(handle.id.clone()))
}

fn read_task_usage(task_root: &Path) -> Result<UsageTotals, DelegationError> {
    let handle = TaskHandle {
        id: task_root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                DelegationError::InvalidParams("task directory has no handle".to_owned())
            })?
            .to_owned(),
    };
    let state_dir = task_root.parent().and_then(Path::parent).ok_or_else(|| {
        DelegationError::InvalidParams("task directory has no state root".to_owned())
    })?;
    read_attempts(state_dir, &handle).map(|(_, _, usage)| usage)
}

fn add_usage(total: &mut UsageTotals, attempt: &UsageTotals) {
    if let Some(cost) = &attempt.cost {
        match &mut total.cost {
            Some(accumulated) if accumulated.currency == cost.currency => {
                accumulated.amount += cost.amount;
            }
            None => total.cost = Some(cost.clone()),
            Some(_) => {}
        }
    }
    if let Some(tokens) = &attempt.tokens {
        match &mut total.tokens {
            Some(accumulated) => {
                accumulated.total_tokens += tokens.total_tokens;
                accumulated.input_tokens += tokens.input_tokens;
                accumulated.output_tokens += tokens.output_tokens;
                add_optional(&mut accumulated.thought_tokens, tokens.thought_tokens);
                add_optional(
                    &mut accumulated.cached_read_tokens,
                    tokens.cached_read_tokens,
                );
                add_optional(
                    &mut accumulated.cached_write_tokens,
                    tokens.cached_write_tokens,
                );
            }
            None => total.tokens = Some(tokens.clone()),
        }
    }
}

fn add_optional(total: &mut Option<u64>, value: Option<u64>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or(0) + value);
    }
}

struct TaskPaths {
    task_root: PathBuf,
    attempt: PathBuf,
    task: PathBuf,
    request: PathBuf,
    state: PathBuf,
    events: PathBuf,
    result: PathBuf,
    supervisor_log: PathBuf,
}

impl TaskPaths {
    fn new(state_dir: &Path, handle: &TaskHandle) -> Self {
        Self::for_attempt(state_dir, handle, 1)
    }

    fn for_attempt(state_dir: &Path, handle: &TaskHandle, number: u32) -> Self {
        let task = state_dir.join("tasks").join(&handle.id);
        let attempt = task.join("attempts").join(number.to_string());
        Self {
            task_root: task.clone(),
            task: task.join("task.json"),
            request: attempt.join("request.json"),
            state: attempt.join("state.json"),
            events: attempt.join("events.jsonl"),
            result: attempt.join("result.json"),
            supervisor_log: attempt.join("supervisor.log"),
            attempt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_binary() -> PathBuf {
        let executable = std::env::current_exe().unwrap_or_else(|error| panic!("exe: {error}"));
        let debug = executable
            .parent()
            .and_then(Path::parent)
            .unwrap_or_else(|| panic!("debug dir"));
        let direct = debug.join("sub-harness-fake");
        if direct.is_file() {
            return direct;
        }
        let sibling = debug
            .parent()
            .and_then(Path::parent)
            .unwrap_or_else(|| panic!("target dir"))
            .join("debug/sub-harness-fake");
        if sibling.is_file() {
            return sibling;
        }
        panic!("sub-harness-fake binary not found");
    }

    fn fake_request(root: &Path, scenario: &str) -> SupervisorRequest {
        SupervisorRequest {
            params: LaunchParams {
                harness: Harness::Codex,
                prompt: "contract probe".to_owned(),
                cwd: root.to_path_buf(),
                harness_binary: fake_binary(),
                model: Some("fixture-model".to_owned()),
                permission_mode: "agent".to_owned(),
            },
            adapter: AdapterLaunch {
                bridge: HarnessLaunch::new(fake_binary())
                    .arg(scenario)
                    .env(
                        "SUB_FAKE_FIXTURES_DIR",
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../sub-harness-fake/fixtures")
                            .to_string_lossy()
                            .into_owned(),
                    )
                    .env(
                        "SUB_FAKE_SCENARIOS_DIR",
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../sub-harness-fake/scenarios")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                session_meta: serde_json::json!({}),
                delegation_guard: "Do not use subagents.".to_owned(),
                resume_mechanism: ResumeMechanism::Resume,
            },
            resume_session_id: None,
        }
    }

    fn prepare_supervisor(
        root: &Path,
        handle: &TaskHandle,
        request: &SupervisorRequest,
    ) -> TaskPaths {
        let paths = TaskPaths::new(root, handle);
        fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
        write_json(&paths.request, request).unwrap_or_else(|error| panic!("request: {error}"));
        paths
    }

    fn prepare_resume_attempt(
        root: &Path,
        handle: &TaskHandle,
        mut request: SupervisorRequest,
        session_id: Option<&str>,
    ) -> TaskPaths {
        let paths = TaskPaths::for_attempt(root, handle, 2);
        fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
        request.resume_session_id = session_id.map(str::to_owned);
        write_json(&paths.request, &request).unwrap_or_else(|error| panic!("request: {error}"));
        paths
    }

    #[tokio::test]
    async fn repeated_wait_reads_same_result() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_111111111111111111111111".to_owned(),
        };
        let paths = TaskPaths::new(root.path(), &handle);
        fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
        write_json(
            &paths.state,
            &ExecutionAttempt {
                number: 1,
                status: TaskStatus::Succeeded,
                supervisor_pid: None,
                supervisor_start_time: None,
                harness_session_id: Some("session".to_owned()),
                usage: UsageTotals::default(),
            },
        )
        .unwrap_or_else(|error| panic!("state: {error}"));
        let result = TaskResult {
            status: TaskStatus::Succeeded,
            summary: "done".to_owned(),
            changed_files: Vec::new(),
            artifacts: Vec::new(),
            harness_session_id: Some("session".to_owned()),
        };
        write_json(&paths.result, &result).unwrap_or_else(|error| panic!("result: {error}"));
        let delegator = Delegator::new(root.path(), "/does/not/run");
        for _ in 0..2 {
            let outcome = delegator
                .wait(&handle, Duration::ZERO)
                .await
                .unwrap_or_else(|error| panic!("wait: {error}"));
            assert_eq!(
                outcome,
                WaitOutcome::Complete {
                    result: result.clone()
                }
            );
        }
    }

    #[tokio::test]
    async fn wait_returns_running_after_timeout() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_222222222222222222222222".to_owned(),
        };
        let paths = TaskPaths::new(root.path(), &handle);
        fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
        write_json(
            &paths.state,
            &ExecutionAttempt {
                number: 1,
                status: TaskStatus::Running,
                supervisor_pid: Some(std::process::id()),
                supervisor_start_time: process_start_time(std::process::id()),
                harness_session_id: None,
                usage: UsageTotals::default(),
            },
        )
        .unwrap_or_else(|error| panic!("state: {error}"));
        let delegator = Delegator::new(root.path(), "/does/not/run");
        let outcome = delegator
            .wait(&handle, Duration::ZERO)
            .await
            .unwrap_or_else(|error| panic!("wait: {error}"));
        assert_eq!(
            outcome,
            WaitOutcome::Running {
                status: TaskStatus::Running
            }
        );
    }

    #[test]
    fn inspect_reports_dead_running_supervisor_as_orphaned() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_999999999999999999999999".to_owned(),
        };
        let paths = TaskPaths::new(root.path(), &handle);
        fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
        let attempt = ExecutionAttempt {
            number: 1,
            status: TaskStatus::Running,
            supervisor_pid: Some(u32::MAX),
            supervisor_start_time: Some(1),
            harness_session_id: Some("fixture-session".to_owned()),
            usage: UsageTotals::default(),
        };
        write_json(&paths.state, &attempt).unwrap_or_else(|error| panic!("state: {error}"));
        write_json(
            &paths.task,
            &DelegatedTask {
                handle: handle.clone(),
                params: LaunchParams {
                    harness: Harness::Codex,
                    prompt: "probe".to_owned(),
                    cwd: root.path().to_path_buf(),
                    harness_binary: std::env::current_exe()
                        .unwrap_or_else(|error| panic!("exe: {error}")),
                    model: None,
                    permission_mode: "agent".to_owned(),
                },
                attempt,
            },
        )
        .unwrap_or_else(|error| panic!("task: {error}"));

        let observed = Delegator::new(root.path(), "/does/not/run")
            .inspect(&handle)
            .unwrap_or_else(|error| panic!("inspect: {error}"));
        assert_eq!(observed.task.status, TaskStatus::Orphaned);
        assert_eq!(observed.attempts[0].status, TaskStatus::Orphaned);
    }

    #[test]
    fn recover_creates_a_sequential_attempt_for_the_recorded_session() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_121212121212121212121212".to_owned(),
        };
        let request = fake_request(root.path(), "replay-minimal");
        let paths = prepare_supervisor(root.path(), &handle, &request);
        let attempt = ExecutionAttempt {
            number: 1,
            status: TaskStatus::Running,
            supervisor_pid: Some(u32::MAX),
            supervisor_start_time: Some(1),
            harness_session_id: Some("fixture-session".to_owned()),
            usage: UsageTotals::default(),
        };
        write_json(&paths.state, &attempt).unwrap_or_else(|error| panic!("state: {error}"));
        write_json(
            &paths.task,
            &DelegatedTask {
                handle: handle.clone(),
                params: request.params,
                attempt,
            },
        )
        .unwrap_or_else(|error| panic!("task: {error}"));

        let recovered = Delegator::new(root.path(), "/bin/true")
            .recover(&handle)
            .unwrap_or_else(|error| panic!("recover: {error}"));
        assert_eq!(recovered.handle, handle);
        assert_eq!(recovered.attempt, 2);
        let next_paths = TaskPaths::for_attempt(root.path(), &handle, 2);
        let next_request: SupervisorRequest =
            read_json(&next_paths.request).unwrap_or_else(|error| panic!("request: {error}"));
        assert_eq!(
            next_request.resume_session_id.as_deref(),
            Some("fixture-session")
        );
        let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
        assert!(
            events
                .iter()
                .any(|event| matches!(event.kind, TaskEventKind::AttemptOrphaned))
        );
    }

    #[test]
    fn derives_changed_file_from_streamed_markdown_link() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let file = root.path().join("proof.txt");
        fs::write(&file, "proof").unwrap_or_else(|error| panic!("write: {error}"));
        let text = format!("Created [proof.txt]({})", file.display());
        assert_eq!(derive_changed_files(&[], &text, root.path()), vec![file]);
    }

    #[test]
    fn ignores_links_outside_working_directory() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        assert!(derive_changed_files(&[], "[outside](/etc/passwd)", root.path()).is_empty());
    }

    #[test]
    fn derives_relative_and_angle_bracket_links() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let file = root.path().join("relative.txt");
        fs::write(&file, "proof").unwrap_or_else(|error| panic!("write: {error}"));
        assert_eq!(
            derive_changed_files(&[], "[file](<relative.txt>)", root.path()),
            vec![file]
        );
        assert!(markdown_destinations("[broken](<missing").is_empty());
        assert!(markdown_destinations("[broken](missing").is_empty());
    }

    #[tokio::test]
    async fn supervisor_derives_success_from_fake_stream() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_333333333333333333333333".to_owned(),
        };
        let paths = prepare_supervisor(
            root.path(),
            &handle,
            &fake_request(root.path(), "replay-minimal"),
        );
        run_supervisor(root.path(), &handle, 1)
            .await
            .unwrap_or_else(|error| panic!("supervisor: {error}"));
        let result: TaskResult =
            read_json(&paths.result).unwrap_or_else(|error| panic!("result: {error}"));
        assert_eq!(result.status, TaskStatus::Succeeded);
        assert!(result.summary.contains("Hello"));
        assert!(result.harness_session_id.is_some());
        assert!(
            fs::read_to_string(paths.events)
                .unwrap_or_else(|error| panic!("events: {error}"))
                .contains("attempt_finished")
        );
    }

    #[tokio::test]
    async fn resume_attempt_succeeds_with_same_session_lineage() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_abababababababababababab".to_owned(),
        };
        let request = fake_request(root.path(), "replay-minimal");
        let session_id = "00000000-0000-0000-0000-000000000001";
        let paths = prepare_resume_attempt(root.path(), &handle, request, Some(session_id));

        run_supervisor(root.path(), &handle, 2)
            .await
            .unwrap_or_else(|error| panic!("supervisor: {error}"));

        let state: ExecutionAttempt =
            read_json(&paths.state).unwrap_or_else(|error| panic!("state: {error}"));
        assert_eq!(state.status, TaskStatus::Succeeded);
        assert_eq!(state.harness_session_id.as_deref(), Some(session_id));
        let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
        assert!(events.iter().any(|event| {
            event.attempt == 2 && matches!(event.kind, TaskEventKind::AttemptResumed)
        }));
    }

    #[tokio::test]
    async fn resume_attempt_fails_when_session_record_is_missing() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_cdcdcdcdcdcdcdcdcdcdcdcd".to_owned(),
        };
        let paths = prepare_resume_attempt(
            root.path(),
            &handle,
            fake_request(root.path(), "replay-minimal"),
            None,
        );

        run_supervisor(root.path(), &handle, 2)
            .await
            .unwrap_or_else(|error| panic!("supervisor: {error}"));

        let result: TaskResult =
            read_json(&paths.result).unwrap_or_else(|error| panic!("result: {error}"));
        assert_eq!(result.status, TaskStatus::Failed);
        let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
        assert!(events.iter().any(|event| matches!(
            event.kind,
            TaskEventKind::AttemptResumeFailed {
                reason: ResumeFailureReason::SessionRecordMissing
            }
        )));
    }

    #[tokio::test]
    async fn resume_attempt_records_harness_refusal() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_efefefefefefefefefefefef".to_owned(),
        };
        let paths = prepare_resume_attempt(
            root.path(),
            &handle,
            fake_request(root.path(), "resume-refused"),
            Some("00000000-0000-0000-0000-000000000001"),
        );

        run_supervisor(root.path(), &handle, 2)
            .await
            .unwrap_or_else(|error| panic!("supervisor: {error}"));

        let result: TaskResult =
            read_json(&paths.result).unwrap_or_else(|error| panic!("result: {error}"));
        assert_eq!(result.status, TaskStatus::Failed);
        let events = read_events(&paths.events).unwrap_or_else(|error| panic!("events: {error}"));
        assert!(events.iter().any(|event| matches!(
            event.kind,
            TaskEventKind::AttemptResumeFailed {
                reason: ResumeFailureReason::HarnessRefused
            }
        )));
    }

    #[tokio::test]
    async fn observe_reports_normalized_events_and_accumulated_usage() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_666666666666666666666666".to_owned(),
        };
        let request = fake_request(root.path(), "replay-codex");
        let paths = prepare_supervisor(root.path(), &handle, &request);
        write_json(
            &paths.task,
            &DelegatedTask {
                handle: handle.clone(),
                params: request.params.clone(),
                attempt: ExecutionAttempt {
                    number: 1,
                    status: TaskStatus::Queued,
                    supervisor_pid: None,
                    supervisor_start_time: None,
                    harness_session_id: None,
                    usage: UsageTotals::default(),
                },
            },
        )
        .unwrap_or_else(|error| panic!("task: {error}"));

        run_supervisor(root.path(), &handle, 1)
            .await
            .unwrap_or_else(|error| panic!("supervisor: {error}"));
        let delegator = Delegator::new(root.path(), "/does/not/run");
        let observed = delegator
            .inspect(&handle)
            .unwrap_or_else(|error| panic!("inspect: {error}"));

        assert_eq!(observed.task.handle, handle);
        assert_eq!(observed.task.usage_support, UsageSupport::codex());
        assert_eq!(
            observed
                .task
                .usage
                .tokens
                .as_ref()
                .map(|usage| usage.total_tokens),
            Some(16_749)
        );
        assert!(observed.task.usage.cost.is_none());
        assert!(
            observed
                .events
                .iter()
                .any(|event| matches!(event.kind, TaskEventKind::UsageAccumulated { .. }))
        );
        let events_json = serde_json::to_string(&observed.events)
            .unwrap_or_else(|error| panic!("events json: {error}"));
        assert!(
            !events_json.contains("Hello"),
            "events must not copy transcript text"
        );

        let listed = delegator
            .list()
            .unwrap_or_else(|error| panic!("list: {error}"));
        assert_eq!(listed.tasks.len(), 1);
        assert_eq!(listed.tasks[0], observed.task);
    }

    #[tokio::test]
    async fn observe_keeps_unreported_usage_absent() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_777777777777777777777777".to_owned(),
        };
        let request = fake_request(root.path(), "replay-minimal");
        let paths = prepare_supervisor(root.path(), &handle, &request);
        write_json(
            &paths.task,
            &DelegatedTask {
                handle: handle.clone(),
                params: request.params.clone(),
                attempt: ExecutionAttempt {
                    number: 1,
                    status: TaskStatus::Queued,
                    supervisor_pid: None,
                    supervisor_start_time: None,
                    harness_session_id: None,
                    usage: UsageTotals::default(),
                },
            },
        )
        .unwrap_or_else(|error| panic!("task: {error}"));
        run_supervisor(root.path(), &handle, 1)
            .await
            .unwrap_or_else(|error| panic!("supervisor: {error}"));

        let observed = Delegator::new(root.path(), "/does/not/run")
            .inspect(&handle)
            .unwrap_or_else(|error| panic!("inspect: {error}"));
        assert!(observed.task.usage.tokens.is_none());
        assert!(observed.task.usage.cost.is_none());
    }

    #[tokio::test]
    async fn observe_accumulates_streamed_cost_and_terminal_tokens() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_888888888888888888888888".to_owned(),
        };
        let mut request = fake_request(root.path(), "replay-usage");
        request.params.harness = Harness::Claude;
        let paths = prepare_supervisor(root.path(), &handle, &request);
        write_json(
            &paths.task,
            &DelegatedTask {
                handle: handle.clone(),
                params: request.params.clone(),
                attempt: ExecutionAttempt {
                    number: 1,
                    status: TaskStatus::Queued,
                    supervisor_pid: None,
                    supervisor_start_time: None,
                    harness_session_id: None,
                    usage: UsageTotals::default(),
                },
            },
        )
        .unwrap_or_else(|error| panic!("task: {error}"));
        run_supervisor(root.path(), &handle, 1)
            .await
            .unwrap_or_else(|error| panic!("supervisor: {error}"));

        let observed = Delegator::new(root.path(), "/does/not/run")
            .inspect(&handle)
            .unwrap_or_else(|error| panic!("inspect: {error}"));
        assert_eq!(observed.task.usage_support, UsageSupport::claude());
        assert_eq!(
            observed.task.usage.cost,
            Some(UsageCost {
                amount: 0.25,
                currency: "USD".to_owned(),
            })
        );
        assert_eq!(
            observed.task.usage.tokens.map(|usage| usage.total_tokens),
            Some(120)
        );
        assert_eq!(
            observed
                .events
                .iter()
                .filter(|event| matches!(event.kind, TaskEventKind::UsageAccumulated { .. }))
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn observe_reads_live_state_without_the_supervisor() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_999999999999999999999999".to_owned(),
        };
        let mut request = fake_request(root.path(), "replay-usage");
        request.params.harness = Harness::Claude;
        let paths = prepare_supervisor(root.path(), &handle, &request);
        write_json(
            &paths.task,
            &DelegatedTask {
                handle: handle.clone(),
                params: request.params.clone(),
                attempt: ExecutionAttempt {
                    number: 1,
                    status: TaskStatus::Queued,
                    supervisor_pid: None,
                    supervisor_start_time: None,
                    harness_session_id: None,
                    usage: UsageTotals::default(),
                },
            },
        )
        .unwrap_or_else(|error| panic!("task: {error}"));
        let state_dir = root.path().to_path_buf();
        let supervisor_handle = handle.clone();
        let supervisor =
            tokio::spawn(async move { run_supervisor(&state_dir, &supervisor_handle, 1).await });
        tokio::time::sleep(Duration::from_millis(100)).await;

        let live = Delegator::new(root.path(), "/does/not/run")
            .inspect(&handle)
            .unwrap_or_else(|error| panic!("live inspect: {error}"));
        assert_eq!(live.task.status, TaskStatus::Running);
        assert!(
            live.events
                .iter()
                .any(|event| matches!(event.kind, TaskEventKind::AttemptStarted))
        );

        supervisor
            .await
            .unwrap_or_else(|error| panic!("join: {error}"))
            .unwrap_or_else(|error| panic!("supervisor: {error}"));
        let complete = Delegator::new(root.path(), "/does/not/run")
            .inspect(&handle)
            .unwrap_or_else(|error| panic!("complete inspect: {error}"));
        assert_eq!(complete.task.status, TaskStatus::Succeeded);
        assert!(complete.task.usage.cost.is_some());
        assert!(complete.task.usage.tokens.is_some());
    }

    #[test]
    fn observe_accumulates_usage_across_attempts() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_aaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        };
        let request = fake_request(root.path(), "replay-minimal");
        for (number, amount, tokens) in [(1, 0.25, 100), (2, 0.50, 200)] {
            let paths = TaskPaths::for_attempt(root.path(), &handle, number);
            fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
            write_json(
                &paths.state,
                &ExecutionAttempt {
                    number,
                    status: TaskStatus::Succeeded,
                    supervisor_pid: None,
                    supervisor_start_time: None,
                    harness_session_id: Some(format!("session-{number}")),
                    usage: UsageTotals {
                        cost: Some(UsageCost {
                            amount,
                            currency: "USD".to_owned(),
                        }),
                        tokens: Some(TurnUsage {
                            total_tokens: tokens,
                            input_tokens: tokens - 10,
                            output_tokens: 10,
                            thought_tokens: None,
                            cached_read_tokens: None,
                            cached_write_tokens: None,
                        }),
                    },
                },
            )
            .unwrap_or_else(|error| panic!("state: {error}"));
        }
        let paths = TaskPaths::new(root.path(), &handle);
        write_json(
            &paths.task,
            &DelegatedTask {
                handle: handle.clone(),
                params: request.params,
                attempt: ExecutionAttempt {
                    number: 1,
                    status: TaskStatus::Succeeded,
                    supervisor_pid: None,
                    supervisor_start_time: None,
                    harness_session_id: Some("session-1".to_owned()),
                    usage: UsageTotals::default(),
                },
            },
        )
        .unwrap_or_else(|error| panic!("task: {error}"));

        let observed = Delegator::new(root.path(), "/does/not/run")
            .inspect(&handle)
            .unwrap_or_else(|error| panic!("inspect: {error}"));
        assert_eq!(observed.attempts.len(), 2);
        assert_eq!(
            observed.task.usage.cost.as_ref().map(|cost| cost.amount),
            Some(0.75)
        );
        assert_eq!(
            observed.task.usage.tokens.map(|usage| usage.total_tokens),
            Some(300)
        );
    }

    #[tokio::test]
    async fn supervisor_persists_failed_result() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_444444444444444444444444".to_owned(),
        };
        let paths = prepare_supervisor(
            root.path(),
            &handle,
            &fake_request(root.path(), "malformed"),
        );
        run_supervisor(root.path(), &handle, 1)
            .await
            .unwrap_or_else(|error| panic!("supervisor state: {error}"));
        let result: TaskResult =
            read_json(&paths.result).unwrap_or_else(|error| panic!("result: {error}"));
        assert_eq!(result.status, TaskStatus::Failed);
        assert!(result.harness_session_id.is_none());
    }

    #[tokio::test]
    async fn supervisor_surfaces_denied_permission() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let handle = TaskHandle {
            id: "tsk_555555555555555555555555".to_owned(),
        };
        let paths = prepare_supervisor(
            root.path(),
            &handle,
            &fake_request(root.path(), "permission-request"),
        );
        run_supervisor(root.path(), &handle, 1)
            .await
            .unwrap_or_else(|error| panic!("supervisor: {error}"));
        let events =
            fs::read_to_string(paths.events).unwrap_or_else(|error| panic!("events: {error}"));
        assert!(events.contains("permission_denied"));
    }

    #[test]
    fn launch_validates_paths_and_persists_handle() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let delegator = Delegator::new(root.path(), "/bin/true");
        let mut request = fake_request(root.path(), "replay-minimal");
        request.params.cwd = PathBuf::from("relative");
        assert!(
            delegator
                .launch(request.params.clone(), request.adapter.clone())
                .is_err()
        );
        request.params.cwd = root.path().to_path_buf();
        request.params.harness_binary = PathBuf::from("relative");
        assert!(
            delegator
                .launch(request.params.clone(), request.adapter.clone())
                .is_err()
        );
        request.params.harness_binary = fake_binary();
        let handle = delegator
            .launch(request.params, request.adapter)
            .unwrap_or_else(|error| panic!("launch: {error}"));
        assert!(handle.id.starts_with("tsk_"));
        assert!(TaskPaths::new(root.path(), &handle).request.is_file());
    }

    #[tokio::test]
    async fn unknown_handle_is_rejected() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let error = Delegator::new(root.path(), "/bin/true")
            .wait(
                &TaskHandle {
                    id: "tsk_000000000000000000000000".to_owned(),
                },
                Duration::ZERO,
            )
            .await
            .err()
            .unwrap_or_else(|| panic!("unknown handle error"));
        assert!(matches!(error, DelegationError::UnknownHandle(_)));
        let invalid = Delegator::new(root.path(), "/bin/true")
            .wait(
                &TaskHandle {
                    id: "../../escape".to_owned(),
                },
                Duration::ZERO,
            )
            .await
            .err()
            .unwrap_or_else(|| panic!("invalid handle"));
        assert!(matches!(invalid, DelegationError::InvalidParams(_)));
    }

    #[test]
    fn status_mapping_covers_terminal_reasons() {
        assert_eq!(status_from_stop(StopReason::EndTurn), TaskStatus::Succeeded);
        assert_eq!(
            status_from_stop(StopReason::Cancelled),
            TaskStatus::Cancelled
        );
        assert_eq!(status_from_stop(StopReason::Refusal), TaskStatus::Failed);
        assert_eq!(status_from_stop(StopReason::MaxTokens), TaskStatus::Failed);
        assert_eq!(
            status_from_stop(StopReason::MaxTurnRequests),
            TaskStatus::Failed
        );
    }

    #[test]
    fn finds_native_session_record_recursively() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let nested = root.path().join("year/month");
        fs::create_dir_all(&nested).unwrap_or_else(|error| panic!("mkdir: {error}"));
        let record = nested.join("session-abc.jsonl");
        fs::write(&record, "{}").unwrap_or_else(|error| panic!("write: {error}"));
        assert_eq!(find_session_record(root.path(), "abc"), Some(record));
        assert_eq!(find_session_record(root.path(), "missing"), None);
    }

    #[test]
    fn native_fallback_uses_each_harness_name() {
        let cwd = Path::new("/path/that/does/not/exist");
        assert!(
            native_session_reference(Harness::Claude, cwd, "missing-claude").contains("claude")
        );
        assert!(native_session_reference(Harness::Codex, cwd, "missing-codex").contains("codex"));
    }

    #[test]
    fn detects_subagent_tool_titles_only() {
        let update = StreamUpdate {
            kind: crate::acp::StreamUpdateKind::ToolCall,
            text: Some("spawn_agent".to_owned()),
            changed_files: Vec::new(),
            cost: None,
        };
        assert!(looks_like_subagent(&update));
        let ordinary = StreamUpdate {
            kind: crate::acp::StreamUpdateKind::AgentMessageChunk,
            text: Some("subagent".to_owned()),
            changed_files: Vec::new(),
            cost: None,
        };
        assert!(!looks_like_subagent(&ordinary));
    }

    #[test]
    fn activity_mapping_covers_the_normalized_vocabulary() {
        let cases = [
            (StreamUpdateKind::AgentMessageChunk, ActivityKind::Message),
            (StreamUpdateKind::AgentThoughtChunk, ActivityKind::Thought),
            (StreamUpdateKind::ToolCall, ActivityKind::ToolCall),
            (
                StreamUpdateKind::ToolCallUpdate,
                ActivityKind::ToolCallUpdate,
            ),
            (
                StreamUpdateKind::SessionInfoUpdate,
                ActivityKind::SessionInfo,
            ),
            (
                StreamUpdateKind::AvailableCommandsUpdate,
                ActivityKind::AvailableCommands,
            ),
            (StreamUpdateKind::Plan, ActivityKind::Plan),
            (
                StreamUpdateKind::PermissionDenied,
                ActivityKind::PermissionDenied,
            ),
            (StreamUpdateKind::Other, ActivityKind::Other),
        ];
        for (update, expected) in cases {
            assert_eq!(activity_kind(update, false), Some(expected));
        }
        assert_eq!(activity_kind(StreamUpdateKind::UsageUpdate, false), None);
        assert_eq!(
            activity_kind(StreamUpdateKind::ToolCall, true),
            Some(ActivityKind::SubagentObserved)
        );
    }
}
