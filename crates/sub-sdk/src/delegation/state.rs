use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::events::{append_event, read_events};
use super::liveness::{effective_status, process_start_time};
use super::supervisor::supervisor_command;
use super::{
    AdapterLaunch, AttemptObservation, CancelDelivery, CancelOutcome, DelegatedTask,
    DelegationError, Delegator, ExecutionAttempt, LaunchParams, SupervisorRequest, TaskEvent,
    TaskEventKind, TaskHandle, TaskInspection, TaskList, TaskOverview, TaskStatus, TurnUsage,
    UsageCost, UsageTotals, WaitOutcome,
};

static HANDLE_COUNTER: AtomicU64 = AtomicU64::new(0);

impl Delegator {
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
        validate_launch_params(&params)?;
        let handle = TaskHandle {
            id: new_handle(&params),
        };
        let paths = TaskPaths::new(&self.state_dir, &handle);
        fs::create_dir_all(&paths.attempt)?;
        let attempt = persist_initial_task(&paths, &handle, params, adapter)?;
        append_event(&paths.events, &handle, 1, TaskEventKind::TaskCreated)?;
        SupervisorSpawner::new(self, &handle, 1, &paths).spawn(attempt)?;
        Ok(handle)
    }

    /// Request cancellation of the latest attempt and return without waiting for completion.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown handle or unreadable persisted state.
    pub fn cancel(&self, handle: &TaskHandle) -> Result<CancelOutcome, DelegationError> {
        validate_handle(handle)?;
        let initial_paths = TaskPaths::new(&self.state_dir, handle);
        if !initial_paths.task.is_file() || !initial_paths.state.is_file() {
            return Err(DelegationError::UnknownHandle(handle.id.clone()));
        }
        let attempt = latest_attempt_number(&self.state_dir, handle)?;
        let paths = TaskPaths::for_attempt(&self.state_dir, handle, attempt);
        let state: ExecutionAttempt = read_json(&paths.state)?;
        let delivery = match effective_status(&state) {
            TaskStatus::Orphaned => CancelDelivery::AttemptOrphaned,
            TaskStatus::Succeeded | TaskStatus::Failed | TaskStatus::Cancelled => {
                CancelDelivery::AlreadyFinished
            }
            TaskStatus::Queued | TaskStatus::Running => {
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(false)
                    .open(&paths.cancel_request)?
                    .sync_all()?;
                CancelDelivery::Delivered
            }
        };
        Ok(CancelOutcome {
            handle: handle.clone(),
            attempt,
            delivery,
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

pub(super) fn new_handle(params: &LaunchParams) -> String {
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

fn validate_launch_params(params: &LaunchParams) -> Result<(), DelegationError> {
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
    Ok(())
}

fn persist_initial_task(
    paths: &TaskPaths,
    handle: &TaskHandle,
    params: LaunchParams,
    adapter: AdapterLaunch,
) -> Result<ExecutionAttempt, DelegationError> {
    let attempt = queued_attempt(1, None);
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
    Ok(attempt)
}

pub(super) fn queued_attempt(number: u32, harness_session_id: Option<String>) -> ExecutionAttempt {
    ExecutionAttempt {
        number,
        status: TaskStatus::Queued,
        supervisor_pid: None,
        supervisor_start_time: None,
        harness_session_id,
        usage: UsageTotals::default(),
    }
}

pub(super) fn validate_handle(handle: &TaskHandle) -> Result<(), DelegationError> {
    let suffix = handle.id.strip_prefix("tsk_").ok_or_else(|| {
        DelegationError::InvalidParams("task handle must use the tsk_ prefix".to_owned())
    })?;
    if !is_lowercase_hex_handle(suffix) {
        return Err(DelegationError::InvalidParams(
            "task handle must contain 24 lowercase hexadecimal characters".to_owned(),
        ));
    }
    Ok(())
}

fn is_lowercase_hex_handle(suffix: &str) -> bool {
    suffix.len() == 24
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> Result<(), DelegationError> {
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temp, path)?;
    Ok(())
}

pub(super) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, DelegationError> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub(super) fn read_attempts(
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

pub(super) fn latest_attempt_number(
    state_dir: &Path,
    handle: &TaskHandle,
) -> Result<u32, DelegationError> {
    let root = state_dir.join("tasks").join(&handle.id).join("attempts");
    fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
        .max()
        .ok_or_else(|| DelegationError::UnknownHandle(handle.id.clone()))
}

pub(super) fn read_task_usage(task_root: &Path) -> Result<UsageTotals, DelegationError> {
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

pub(super) fn add_usage(total: &mut UsageTotals, attempt: &UsageTotals) {
    add_cost(&mut total.cost, attempt.cost.as_ref());
    if let Some(tokens) = &attempt.tokens {
        add_tokens(&mut total.tokens, tokens);
    }
}

fn add_cost(total: &mut Option<UsageCost>, cost: Option<&UsageCost>) {
    match (total, cost) {
        (Some(accumulated), Some(value)) if accumulated.currency == value.currency => {
            accumulated.amount += value.amount;
        }
        (slot @ None, Some(value)) => *slot = Some(value.clone()),
        _ => {}
    }
}

fn add_tokens(total: &mut Option<TurnUsage>, tokens: &TurnUsage) {
    let Some(accumulated) = total else {
        *total = Some(tokens.clone());
        return;
    };
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

pub(super) struct SupervisorSpawner<'a> {
    executable: &'a Path,
    state_dir: &'a Path,
    handle: &'a TaskHandle,
    number: u32,
    paths: &'a TaskPaths,
}

impl<'a> SupervisorSpawner<'a> {
    pub(super) fn new(
        delegator: &'a Delegator,
        handle: &'a TaskHandle,
        number: u32,
        paths: &'a TaskPaths,
    ) -> Self {
        Self {
            executable: &delegator.supervisor_executable,
            state_dir: &delegator.state_dir,
            handle,
            number,
            paths,
        }
    }

    pub(super) fn spawn(&self, queued: ExecutionAttempt) -> Result<(), DelegationError> {
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.paths.supervisor_log)?;
        let mut command = supervisor_command(self.executable);
        let child = command
            .args([
                "__supervise",
                &self.handle.id,
                &self.number.to_string(),
                "--state-dir",
            ])
            .arg(self.state_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        write_json(
            &self.paths.state,
            &ExecutionAttempt {
                supervisor_pid: Some(child.id()),
                supervisor_start_time: process_start_time(child.id()),
                ..queued
            },
        )
    }
}

fn add_optional(total: &mut Option<u64>, value: Option<u64>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or(0) + value);
    }
}

pub(super) struct TaskPaths {
    pub(super) task_root: PathBuf,
    pub(super) attempt: PathBuf,
    pub(super) task: PathBuf,
    pub(super) request: PathBuf,
    pub(super) state: PathBuf,
    pub(super) events: PathBuf,
    pub(super) result: PathBuf,
    pub(super) supervisor_log: PathBuf,
    pub(super) cancel_request: PathBuf,
}

impl TaskPaths {
    pub(super) fn new(state_dir: &Path, handle: &TaskHandle) -> Self {
        Self::for_attempt(state_dir, handle, 1)
    }

    pub(super) fn for_attempt(state_dir: &Path, handle: &TaskHandle, number: u32) -> Self {
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
            cancel_request: attempt.join("cancel.request"),
            attempt,
        }
    }
}
