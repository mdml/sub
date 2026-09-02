use std::fs;
#[cfg(not(target_os = "linux"))]
use std::process::Command;

use super::{ExecutionAttempt, TaskStatus};

pub(super) fn effective_status(attempt: &ExecutionAttempt) -> TaskStatus {
    if attempt.status == TaskStatus::Running && !supervisor_is_alive(attempt) {
        TaskStatus::Orphaned
    } else {
        attempt.status
    }
}

#[cfg(target_os = "linux")]
pub(super) fn process_start_time(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields = stat
        .rsplit_once(") ")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    fields.get(19)?.parse().ok()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn process_start_time(_pid: u32) -> Option<u64> {
    None
}

pub(super) fn supervisor_is_alive(attempt: &ExecutionAttempt) -> bool {
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
