use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use sub_sdk::delegation::{
    CancelDelivery, DelegationError, Delegator, HarnessChild, OrphanedChildDisposition,
    TaskEventKind, TaskHandle, TaskStatus, WaitOutcome,
};

use super::*;

/// The harness child of an orphaned attempt outlives its supervisor until cancel ends it.
///
/// The harness is spawned exactly as the supervisor would spawn it, as the leader of its own
/// process group with stdin held open, so it waits for ACP traffic that never comes. The state
/// records a dead supervisor and the child's verified identity, as the supervisor would have left
/// behind. Cancel must terminate the child, publish a cancelled result, and reject recovery.
#[tokio::test(flavor = "current_thread")]
async fn cancel_orphaned_terminates_live_child() {
    let harness = ContractHarness::select(FakeScenario::Hang);
    let launch = harness.launch();
    let mut command = Command::new(launch.command());
    command
        .args(launch.args())
        .envs(launch.environment())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("spawn harness: {error}"));
    let root = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_676767676767676767676767".to_owned(),
    };
    write_orphaned_state(root.path(), &handle.id, child.id(), launch.command());
    let delegator = Delegator::new(root.path(), "/does/not/run");

    let outcome = delegator
        .cancel(&handle)
        .unwrap_or_else(|error| panic!("cancel: {error}"));

    assert_eq!(outcome.delivery, CancelDelivery::AttemptOrphaned);
    let status = child
        .wait()
        .unwrap_or_else(|error| panic!("wait for harness: {error}"));
    assert!(
        !status.success(),
        "the orphaned child must have been signalled"
    );
    let inspection = delegator
        .inspect(&handle)
        .unwrap_or_else(|error| panic!("inspect: {error}"));
    assert_eq!(inspection.task.status, TaskStatus::Cancelled);
    assert!(inspection.events.iter().any(|event| event.kind
        == TaskEventKind::OrphanedChildDisposed {
            disposition: OrphanedChildDisposition::Terminated
        }));
    assert!(inspection.events.iter().any(|event| event.kind
        == TaskEventKind::AttemptFinished {
            status: TaskStatus::Cancelled
        }));
    let waited = delegator
        .wait(&handle, Duration::ZERO)
        .await
        .unwrap_or_else(|error| panic!("wait: {error}"));
    assert!(
        matches!(waited, WaitOutcome::Complete { result } if result.status == TaskStatus::Cancelled)
    );
    assert!(matches!(
        delegator.recover(&handle),
        Err(DelegationError::NotOrphaned(_))
    ));
}

/// Persist what a supervisor leaves behind when it dies mid-attempt after spawning its child.
fn write_orphaned_state(root: &Path, handle: &str, child_pid: u32, harness_binary: &Path) {
    let attempt = root.join("tasks").join(handle).join("attempts/1");
    fs::create_dir_all(&attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let params = serde_json::json!({
        "harness": "codex",
        "prompt": "contract probe",
        "cwd": root,
        "harness_binary": harness_binary,
        "model": null,
        "permission_mode": "agent"
    });
    let state = serde_json::json!({
        "number": 1,
        "status": "running",
        "supervisor_pid": u32::MAX,
        "supervisor_start_time": 1,
        "harness_session_id": "fixture-session",
        "harness_child": HarnessChild::observe(child_pid)
            .unwrap_or_else(|| panic!("harness child identity")),
        "usage": {"cost": null, "tokens": null}
    });
    write(&attempt.join("state.json"), &state);
    write(
        &root.join("tasks").join(handle).join("task.json"),
        &serde_json::json!({"handle": {"id": handle}, "params": params, "attempt": state}),
    );
}

fn write(path: &Path, value: &serde_json::Value) {
    fs::write(
        path,
        serde_json::to_vec(value).unwrap_or_else(|error| panic!("json: {error}")),
    )
    .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}
