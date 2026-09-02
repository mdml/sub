use std::fs;

use super::events::append_event;
use super::liveness::effective_status;
use super::state::{
    SupervisorSpawner, TaskPaths, latest_attempt_number, queued_attempt, read_json,
    validate_handle, write_json,
};
use super::{
    DelegationError, Delegator, ExecutionAttempt, RecoverOutcome, RecoveryRejectionReason,
    SupervisorRequest, TaskEventKind, TaskHandle, TaskStatus,
};

impl Delegator {
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
        require_orphaned(&prior_paths, handle, prior_number, &prior)?;
        append_event(
            &prior_paths.events,
            handle,
            prior_number,
            TaskEventKind::AttemptOrphaned,
        )?;

        let number = prior_number + 1;
        let paths = TaskPaths::for_attempt(&self.state_dir, handle, number);
        fs::create_dir(&paths.attempt)?;
        let queued = prepare_recovery(&prior_paths, &paths, &prior, number)?;
        SupervisorSpawner::new(self, handle, number, &paths).spawn(queued)?;
        Ok(RecoverOutcome {
            handle: handle.clone(),
            attempt: number,
        })
    }
}

fn require_orphaned(
    paths: &TaskPaths,
    handle: &TaskHandle,
    number: u32,
    prior: &ExecutionAttempt,
) -> Result<(), DelegationError> {
    let status = effective_status(prior);
    if status == TaskStatus::Cancelled {
        append_event(
            &paths.events,
            handle,
            number,
            TaskEventKind::AttemptRecoveryRejected {
                reason: RecoveryRejectionReason::Cancelled,
            },
        )?;
    }
    if status != TaskStatus::Orphaned {
        return Err(DelegationError::NotOrphaned(handle.id.clone()));
    }
    Ok(())
}

fn prepare_recovery(
    prior_paths: &TaskPaths,
    paths: &TaskPaths,
    prior: &ExecutionAttempt,
    number: u32,
) -> Result<ExecutionAttempt, DelegationError> {
    let mut request: SupervisorRequest = read_json(&prior_paths.request)?;
    request
        .resume_session_id
        .clone_from(&prior.harness_session_id);
    write_json(&paths.request, &request)?;
    let queued = queued_attempt(number, prior.harness_session_id.clone());
    write_json(&paths.state, &queued)?;
    Ok(queued)
}
