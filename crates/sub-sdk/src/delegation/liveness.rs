#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "macos")]
use std::mem::{MaybeUninit, size_of};

use super::{ExecutionAttempt, TaskStatus};

pub(super) fn effective_status(attempt: &ExecutionAttempt) -> TaskStatus {
    if attempt.status == TaskStatus::Running && !supervisor_is_alive(attempt) {
        TaskStatus::Orphaned
    } else {
        attempt.status
    }
}

pub(super) fn process_start_time(pid: u32) -> Option<u64> {
    process_identity(pid).map(|identity| identity.start_time)
}

struct ProcessIdentity {
    start_time: u64,
    alive: bool,
}

pub(super) fn supervisor_is_alive(attempt: &ExecutionAttempt) -> bool {
    let Some(pid) = attempt.supervisor_pid else {
        return false;
    };
    let Some(expected) = attempt.supervisor_start_time else {
        return false;
    };
    process_identity(pid).is_some_and(|identity| identity.alive && identity.start_time == expected)
}

#[cfg(target_os = "linux")]
fn process_identity(pid: u32) -> Option<ProcessIdentity> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    linux_process_identity(&stat)
}

#[cfg(target_os = "linux")]
fn linux_process_identity(stat: &str) -> Option<ProcessIdentity> {
    let fields = stat
        .rsplit_once(") ")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    Some(ProcessIdentity {
        alive: fields.first().is_some_and(|state| *state != "Z"),
        start_time: fields.get(19)?.parse().ok()?,
    })
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn process_identity(pid: u32) -> Option<ProcessIdentity> {
    let mut info = MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let size = i32::try_from(size_of::<libc::proc_bsdinfo>()).ok()?;
    let read = unsafe {
        libc::proc_pidinfo(
            i32::try_from(pid).ok()?,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if read != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some(ProcessIdentity {
        start_time: info
            .pbi_start_tvsec
            .checked_mul(1_000_000)?
            .checked_add(info.pbi_start_tvusec)?,
        alive: info.pbi_status != libc::SZOMB,
    })
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
compile_error!("sub supports process recovery only on Linux and macOS");

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    use super::linux_process_identity;

    #[test]
    fn parses_linux_start_identity_after_parenthesized_name() {
        let stat = "42 (name with ) paren) R 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 12345";
        let identity = linux_process_identity(stat).unwrap_or_else(|| panic!("identity"));
        assert!(identity.alive);
        assert_eq!(identity.start_time, 12_345);
    }

    #[test]
    fn rejects_linux_zombie_as_live() {
        let stat = "42 (zombie) Z 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 12345";
        let identity = linux_process_identity(stat).unwrap_or_else(|| panic!("identity"));
        assert!(!identity.alive);
    }
}
