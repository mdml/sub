//! Disposal of the harness child of an attempt whose supervisor is gone.
//!
//! No supervisor can deliver ACP cancellation for an orphaned attempt, so the kernel deals with
//! the recorded child itself. Terminal cancel then publishes a cancelled result and records the
//! same terminal events a live cancel would, so recovery is rejected afterwards; recover reuses
//! only the disposal, so a recovered task has one live child.

use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use super::events::append_event;
use super::liveness::{ProcessCheck, process_check};
use super::result::{base_artifacts, native_session_reference};
use super::state::{TaskPaths, read_json, write_json};
use super::supervisor::CANCEL_GRACE_PERIOD;
use super::{
    ArtifactKind, ArtifactReference, DelegatedTask, DelegationError, ExecutionAttempt,
    HarnessChild, OrphanedChildDisposition, TaskEventKind, TaskHandle, TaskResult, TaskStatus,
};

const TERMINATION_POLL: Duration = Duration::from_millis(25);

/// Cancel an orphaned attempt: end its child where safely possible, then make the task terminal.
pub(super) fn cancel_orphaned_attempt(
    state_dir: &Path,
    handle: &TaskHandle,
    paths: &TaskPaths,
    attempt: ExecutionAttempt,
) -> Result<(), DelegationError> {
    let number = attempt.number;
    append_event(
        &paths.events,
        handle,
        number,
        TaskEventKind::AttemptOrphaned,
    )?;
    let disposition = dispose_of_child(attempt.harness_child.as_ref(), CANCEL_GRACE_PERIOD);
    append_event(
        &paths.events,
        handle,
        number,
        TaskEventKind::OrphanedChildDisposed { disposition },
    )?;
    let task: DelegatedTask = read_json(&TaskPaths::new(state_dir, handle).task)?;
    let result = cancelled_result(paths, &task, &attempt, disposition);
    write_json(&paths.result, &result)?;
    write_json(
        &paths.state,
        &ExecutionAttempt {
            status: TaskStatus::Cancelled,
            ..attempt
        },
    )?;
    append_event(
        &paths.events,
        handle,
        number,
        TaskEventKind::AttemptCancelled {
            harness_honored: false,
        },
    )?;
    append_event(
        &paths.events,
        handle,
        number,
        TaskEventKind::AttemptFinished {
            status: TaskStatus::Cancelled,
        },
    )
}

fn cancelled_result(
    paths: &TaskPaths,
    task: &DelegatedTask,
    attempt: &ExecutionAttempt,
    disposition: OrphanedChildDisposition,
) -> TaskResult {
    let mut artifacts = base_artifacts(paths);
    if let Some(session_id) = &attempt.harness_session_id {
        artifacts.push(ArtifactReference {
            kind: ArtifactKind::NativeSession,
            location: native_session_reference(task.params.harness, &task.params.cwd, session_id),
        });
    }
    TaskResult {
        status: TaskStatus::Cancelled,
        summary: format!(
            "cancelled while orphaned: the attempt's supervisor died before it finished, so no \
             result was derived from the child's stream; harness child {}",
            describe(disposition)
        ),
        changed_files: Vec::new(),
        artifacts,
        harness_session_id: attempt.harness_session_id.clone(),
    }
}

const fn describe(disposition: OrphanedChildDisposition) -> &'static str {
    match disposition {
        OrphanedChildDisposition::Terminated => "was terminated",
        OrphanedChildDisposition::SurvivedTermination => "survived termination",
        OrphanedChildDisposition::AlreadyGone => "was already gone",
        OrphanedChildDisposition::IdentityUnverified => {
            "identity could not be verified, so it was not signalled"
        }
        OrphanedChildDisposition::IdentityUnrecorded => "identity was never recorded",
    }
}

/// Signal the recorded child with the live-cancel grace and force semantics, or explain why not.
pub(super) fn dispose_of_child(
    child: Option<&HarnessChild>,
    grace_period: Duration,
) -> OrphanedChildDisposition {
    let Some(child) = child else {
        return OrphanedChildDisposition::IdentityUnrecorded;
    };
    match process_check(child.pid, child.start_time) {
        ProcessCheck::Gone => OrphanedChildDisposition::AlreadyGone,
        ProcessCheck::Mismatch => OrphanedChildDisposition::IdentityUnverified,
        ProcessCheck::Live => terminate(child, grace_period),
    }
}

fn terminate(child: &HarnessChild, grace_period: Duration) -> OrphanedChildDisposition {
    if signal(child, libc::SIGTERM).is_ok() && wait_until_gone(child, grace_period) {
        return OrphanedChildDisposition::Terminated;
    }
    if signal(child, libc::SIGKILL).is_ok() && wait_until_gone(child, grace_period) {
        return OrphanedChildDisposition::Terminated;
    }
    OrphanedChildDisposition::SurvivedTermination
}

/// Signal the verified child, reaching its whole process group only when it leads that group.
#[allow(unsafe_code)]
fn signal(child: &HarnessChild, signal: libc::c_int) -> io::Result<()> {
    let pid = i32::try_from(child.pid)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "child PID exceeds i32"))?;
    let target = if child.process_group == Some(child.pid) {
        -pid
    } else {
        pid
    };
    if unsafe { libc::kill(target, signal) } == -1 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    Ok(())
}

fn wait_until_gone(child: &HarnessChild, bound: Duration) -> bool {
    let deadline = Instant::now() + bound;
    loop {
        if process_check(child.pid, child.start_time) != ProcessCheck::Live {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(TERMINATION_POLL);
    }
}
