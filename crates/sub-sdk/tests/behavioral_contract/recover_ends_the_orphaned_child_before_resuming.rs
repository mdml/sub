use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use sub_sdk::delegation::{
    Delegator, HarnessChild, OrphanedChildDisposition, TaskEventKind, TaskHandle,
};

use super::*;
use crate::common::fake_binary::fake_binary;

/// A stand-in supervisor for attempt 2: recover must dispose of the orphan before it spawns.
///
/// The fake-harness binary stands in because `/bin/true` cannot be spawned as a detached
/// session on the macOS CI runner.
fn supervisor_stand_in() -> std::path::PathBuf {
    fake_binary()
}

/// Recover ends the orphaned attempt's live child, so the recovered task has one live child.
///
/// The harness is spawned exactly as the supervisor would spawn it, as the leader of its own
/// process group with stdin held open, so it waits for ACP traffic that never comes. The state
/// records a dead supervisor and the child's verified identity. Recover must terminate that child
/// and record the disposition on attempt 1 before attempt 2 exists.
#[tokio::test(flavor = "current_thread")]
async fn recover_ends_the_live_orphaned_child_before_resuming() {
    let harness = ContractHarness::select(FakeScenario::Hang);
    let mut child = spawn_harness(&harness);
    let root = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_686868686868686868686868".to_owned(),
    };
    let identity =
        HarnessChild::observe(child.id()).unwrap_or_else(|| panic!("harness child identity"));
    write_orphaned_state(root.path(), &handle.id, &harness, Some(identity));

    let recovered = Delegator::new(root.path(), supervisor_stand_in())
        .recover(&handle)
        .unwrap_or_else(|error| panic!("recover: {error}"));

    assert_eq!(recovered.attempt, 2);
    let status = child
        .wait()
        .unwrap_or_else(|error| panic!("wait for harness: {error}"));
    assert!(
        !status.success(),
        "the orphaned child must be gone before attempt 2 resumes the session"
    );
    assert_eq!(
        disposition(&root, &handle),
        OrphanedChildDisposition::Terminated
    );
}

/// An orphaned child that died with its supervisor is recorded and left alone.
#[tokio::test(flavor = "current_thread")]
async fn recover_records_an_orphaned_child_that_is_already_gone() {
    let harness = ContractHarness::select(FakeScenario::Hang);
    let mut child = spawn_harness(&harness);
    let identity =
        HarnessChild::observe(child.id()).unwrap_or_else(|| panic!("harness child identity"));
    child.kill().unwrap_or_else(|error| panic!("kill: {error}"));
    let _ = child.wait();
    let root = TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = TaskHandle {
        id: "tsk_696969696969696969696969".to_owned(),
    };
    write_orphaned_state(root.path(), &handle.id, &harness, Some(identity));

    let recovered = Delegator::new(root.path(), supervisor_stand_in())
        .recover(&handle)
        .unwrap_or_else(|error| panic!("recover: {error}"));

    assert_eq!(recovered.attempt, 2);
    assert_eq!(
        disposition(&root, &handle),
        OrphanedChildDisposition::AlreadyGone
    );
}

/// Spawn the harness the way a supervisor does: own process group, stdin held open.
fn spawn_harness(harness: &ContractHarness) -> std::process::Child {
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
    command
        .spawn()
        .unwrap_or_else(|error| panic!("spawn harness: {error}"))
}

/// What recover recorded about attempt 1's child.
fn disposition(root: &TempDir, handle: &TaskHandle) -> OrphanedChildDisposition {
    let inspection = Delegator::new(root.path(), supervisor_stand_in())
        .inspect(handle)
        .unwrap_or_else(|error| panic!("inspect: {error}"));
    inspection
        .events
        .iter()
        .find_map(|event| match event.kind {
            TaskEventKind::OrphanedChildDisposed { disposition } => Some(disposition),
            _ => None,
        })
        .unwrap_or_else(|| panic!("recover must record an orphaned-child disposition"))
}

/// Persist what a supervisor leaves behind when it dies mid-attempt after spawning its child.
fn write_orphaned_state(
    root: &Path,
    handle: &str,
    harness: &ContractHarness,
    child: Option<HarnessChild>,
) {
    let attempt = root.join("tasks").join(handle).join("attempts/1");
    fs::create_dir_all(&attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let launch = harness.launch();
    let params = serde_json::json!({
        "harness": "codex",
        "prompt": "contract probe",
        "cwd": root,
        "harness_binary": launch.command(),
        "model": null,
        "permission_mode": "agent"
    });
    let state = serde_json::json!({
        "number": 1,
        "status": "running",
        "supervisor_pid": u32::MAX,
        "supervisor_start_time": 1,
        "harness_session_id": "fixture-session",
        "harness_child": child,
        "usage": {"cost": null, "tokens": null}
    });
    write(&attempt.join("state.json"), &state);
    write(
        &attempt.join("request.json"),
        &serde_json::json!({
            "params": params,
            "adapter": {
                "bridge": launch,
                "session_meta": {},
                "delegation_guard": "Do not use subagents.",
                "resume_mechanism": "resume"
            },
            "resume_session_id": null
        }),
    );
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
