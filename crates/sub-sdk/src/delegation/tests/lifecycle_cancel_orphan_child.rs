use std::process::{Command, Stdio};

use super::liveness::process_group;
use super::orphan::dispose_of_child;
use super::*;

fn spawn_child() -> std::process::Child {
    Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn: {error}"))
}

fn identity_of(child: &std::process::Child) -> HarnessChild {
    HarnessChild {
        pid: child.id(),
        start_time: process_start_time(child.id()).unwrap_or_else(|| panic!("start time")),
        process_group: process_group(child.id()),
    }
}

#[test]
fn unverified_identity_is_recorded_and_left_alone() {
    let mut child = spawn_child();
    let recorded = HarnessChild {
        start_time: identity_of(&child).start_time.wrapping_add(1),
        ..identity_of(&child)
    };

    let disposition = dispose_of_child(Some(&recorded), Duration::from_millis(100));

    assert_eq!(disposition, OrphanedChildDisposition::IdentityUnverified);
    assert!(
        child
            .try_wait()
            .unwrap_or_else(|error| panic!("try_wait: {error}"))
            .is_none(),
        "an unverified PID must not be signalled"
    );
    child.kill().unwrap_or_else(|error| panic!("kill: {error}"));
    let _ = child.wait();
}

#[test]
fn already_gone_child_is_reported_without_signalling() {
    let mut child = spawn_child();
    let recorded = identity_of(&child);
    child.kill().unwrap_or_else(|error| panic!("kill: {error}"));
    let _ = child.wait();

    assert_eq!(
        dispose_of_child(Some(&recorded), Duration::from_millis(100)),
        OrphanedChildDisposition::AlreadyGone
    );
}

#[test]
fn live_child_is_terminated_within_the_grace_period() {
    let mut child = spawn_child();
    let recorded = identity_of(&child);

    let disposition = dispose_of_child(Some(&recorded), Duration::from_secs(2));

    assert_eq!(disposition, OrphanedChildDisposition::Terminated);
    let status = child.wait().unwrap_or_else(|error| panic!("wait: {error}"));
    assert!(!status.success());
}
