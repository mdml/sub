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
    AcpClient, AcpClientConfig, HarnessLaunch, PromptOptions, StopReason, StreamUpdate,
    UpdateObserver,
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

/// One semantic delegated task and its single beta execution attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegatedTask {
    /// Stable task identity used by controls.
    pub handle: TaskHandle,
    /// Inputs supplied by the manager.
    pub params: LaunchParams,
    /// The task's sole execution attempt in the beta.
    pub attempt: ExecutionAttempt,
}

/// One process invocation of a harness under a delegated task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionAttempt {
    /// Attempt number, always `1` in the beta.
    pub number: u32,
    /// Latest durable lifecycle status.
    pub status: TaskStatus,
    /// Supervisor process identifier when known.
    pub supervisor_pid: Option<u32>,
    /// Vendor-owned session identifier once session creation succeeds.
    pub harness_session_id: Option<String>,
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
}

/// Lifecycle status derived and persisted by `sub`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Supervisor has not started the ACP connection yet.
    Queued,
    /// The single beta execution attempt is active.
    Running,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WaitOutcome {
    /// The task did not reach a terminal state before the timeout.
    Running {
        /// Latest persisted lifecycle status.
        status: TaskStatus,
    },
    /// The task has a durable result.
    Complete {
        /// Derived result handoff.
        result: TaskResult,
    },
}

/// One append-only event record written by the supervisor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    /// Unix time in milliseconds.
    pub timestamp_unix_ms: u128,
    /// Stable normalized event name.
    pub kind: String,
    /// Stream detail when present.
    pub update: Option<StreamUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SupervisorRequest {
    params: LaunchParams,
    adapter: AdapterLaunch,
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
            harness_session_id: None,
        };
        write_json(
            &paths.task,
            &DelegatedTask {
                handle: handle.clone(),
                params: params.clone(),
                attempt: attempt.clone(),
            },
        )?;
        write_json(&paths.request, &SupervisorRequest { params, adapter })?;
        write_json(&paths.state, &attempt)?;
        append_event(&paths.events, "task_created", None)?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.supervisor_log)?;
        let mut command = supervisor_command(&self.supervisor_executable);
        let child = command
            .args(["__supervise", &handle.id, "--state-dir"])
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
                harness_session_id: None,
            },
        )?;
        Ok(handle)
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
        let paths = TaskPaths::new(&self.state_dir, handle);
        if !paths.state.is_file() {
            return Err(DelegationError::UnknownHandle(handle.id.clone()));
        }
        let deadline = Instant::now() + timeout;
        loop {
            if paths.result.is_file() {
                return Ok(WaitOutcome::Complete {
                    result: read_json(&paths.result)?,
                });
            }
            let state: ExecutionAttempt = read_json(&paths.state)?;
            if Instant::now() >= deadline {
                return Ok(WaitOutcome::Running {
                    status: state.status,
                });
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
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

/// Run the one-attempt supervisor for a previously launched handle.
///
/// # Errors
///
/// Returns an error when persisted state cannot be read or written.
pub async fn run_supervisor(state_dir: &Path, handle: &TaskHandle) -> Result<(), DelegationError> {
    validate_handle(handle)?;
    let paths = TaskPaths::new(state_dir, handle);
    let request: SupervisorRequest = read_json(&paths.request)?;
    write_json(
        &paths.state,
        &ExecutionAttempt {
            number: 1,
            status: TaskStatus::Running,
            supervisor_pid: Some(std::process::id()),
            harness_session_id: None,
        },
    )?;
    append_event(&paths.events, "attempt_started", None)?;

    let events = Arc::new(Mutex::new(paths.events.clone()));
    let observer: UpdateObserver = Arc::new(move |update| {
        if let Ok(path) = events.lock() {
            let subagent_observed = looks_like_subagent(&update);
            let _ = append_event(&path, "stream_update", Some(update));
            if subagent_observed {
                let _ = append_event(&path, "subagent_observed", None);
            }
        }
    });
    let prompt = format!(
        "{}\n\n{}",
        request.params.prompt, request.adapter.delegation_guard
    );
    let client = AcpClient::new(request.adapter.bridge, AcpClientConfig::default());
    let outcome = client
        .prompt_turn_observing(
            &request.params.cwd,
            &prompt,
            PromptOptions {
                permission_mode: Some(request.params.permission_mode),
                model: request.params.model,
                session_meta: Some(request.adapter.session_meta),
                ..PromptOptions::default()
            },
            Some(observer),
        )
        .await;

    let (result, terminal_event) = match outcome {
        Ok((session, prompt_result)) => {
            let status = status_from_stop(prompt_result.stop_reason);
            let changed_files = derive_changed_files(
                &prompt_result.updates,
                &prompt_result.final_text,
                &request.params.cwd,
            );
            let artifacts = artifacts(
                &paths,
                request.params.harness,
                &request.params.cwd,
                &session.session_id,
            );
            (
                TaskResult {
                    status,
                    summary: prompt_result.final_text.trim().to_owned(),
                    changed_files,
                    artifacts,
                    harness_session_id: Some(session.session_id),
                },
                "attempt_finished",
            )
        }
        Err(error) => (
            TaskResult {
                status: TaskStatus::Failed,
                summary: error.to_string(),
                changed_files: Vec::new(),
                artifacts: base_artifacts(&paths),
                harness_session_id: None,
            },
            "attempt_failed",
        ),
    };
    append_event(&paths.events, terminal_event, None)?;
    write_json(&paths.result, &result)?;
    write_json(
        &paths.state,
        &ExecutionAttempt {
            number: 1,
            status: result.status,
            supervisor_pid: Some(std::process::id()),
            harness_session_id: result.harness_session_id.clone(),
        },
    )?;
    Ok(())
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
    kind: &str,
    update: Option<StreamUpdate>,
) -> Result<(), DelegationError> {
    let event = TaskEvent {
        timestamp_unix_ms: now_ms(),
        kind: kind.to_owned(),
        update,
    };
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, &event)?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

struct TaskPaths {
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
        let task = state_dir.join("tasks").join(&handle.id);
        let attempt = task.join("attempts/1");
        Self {
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
            },
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
                harness_session_id: Some("session".to_owned()),
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
                supervisor_pid: None,
                harness_session_id: None,
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
        run_supervisor(root.path(), &handle)
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
        run_supervisor(root.path(), &handle)
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
        run_supervisor(root.path(), &handle)
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
        };
        assert!(looks_like_subagent(&update));
        let ordinary = StreamUpdate {
            kind: crate::acp::StreamUpdateKind::AgentMessageChunk,
            text: Some("subagent".to_owned()),
            changed_files: Vec::new(),
        };
        assert!(!looks_like_subagent(&ordinary));
    }
}
