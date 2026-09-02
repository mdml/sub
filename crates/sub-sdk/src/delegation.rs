//! Durable delegated-task launch, supervision, observation, recovery, cancellation, and wait semantics.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::acp::{HarnessLaunch, TurnUsage};

mod events;
mod liveness;
mod recovery;
mod result;
mod state;
mod supervisor;

/// Supported child harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Harness {
    /// Claude Code.
    Claude,
    /// `OpenAI Codex`.
    Codex,
    /// Cursor Agent.
    #[serde(rename = "cursor", alias = "cursor_agent")]
    CursorAgent,
}

impl Harness {
    pub(super) fn usage_support(self) -> UsageSupport {
        match self {
            Self::Claude => UsageSupport::claude(),
            Self::Codex => UsageSupport::codex(),
            Self::CursorAgent => UsageSupport::cursor_agent(),
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

    /// Cursor Agent support verified by the real-harness contract suite.
    #[must_use]
    pub const fn cursor_agent() -> Self {
        Self {
            cost: false,
            tokens: false,
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
    AttemptCancelled {
        /// Whether the harness acknowledged ACP cancellation within the grace period.
        harness_honored: bool,
    },
    /// Recovery was rejected before a new attempt because the task is terminal.
    AttemptRecoveryRejected {
        /// Stable terminal reason.
        reason: RecoveryRejectionReason,
    },
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

/// Why a terminal task cannot be recovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryRejectionReason {
    /// Cancellation is a final delegated-task outcome.
    Cancelled,
}

/// Immediate disposition of a cancel request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelDelivery {
    /// The live supervisor can observe the durable request marker.
    Delivered,
    /// The latest attempt already has a terminal status.
    AlreadyFinished,
    /// The latest attempt lost its supervisor.
    AttemptOrphaned,
}

/// Immediate response from explicit cancellation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelOutcome {
    /// Stable delegated-task handle.
    pub handle: TaskHandle,
    /// Latest attempt targeted by the request.
    pub attempt: u32,
    /// What the kernel did with the request.
    pub delivery: CancelDelivery,
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
pub(super) struct SupervisorRequest {
    pub(super) params: LaunchParams,
    pub(super) adapter: AdapterLaunch,
    #[serde(default)]
    pub(super) resume_session_id: Option<String>,
}

/// Delegation kernel bound to a private state directory and supervisor executable.
#[derive(Debug, Clone)]
pub struct Delegator {
    pub(super) state_dir: PathBuf,
    pub(super) supervisor_executable: PathBuf,
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
}

pub use supervisor::run_supervisor;

#[cfg(test)]
use events::{activity_kind, read_events};
#[cfg(test)]
use liveness::supervisor_is_alive;
#[cfg(test)]
use result::{
    find_session_record, looks_like_subagent, native_session_reference, status_from_stop,
};
#[cfg(test)]
use state::{
    TaskPaths, add_usage, read_attempts, read_json, read_task_usage, validate_handle, write_json,
};

#[cfg(test)]
mod tests;
