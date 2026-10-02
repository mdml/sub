//! Smoke test: the `sub-mcp` binary runs and reports its version.

use std::io::Write;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

/// Run `sub-mcp` with configuration discovery pinned to an absent file under `root`, so tests never read the developer's real `sub.toml`.
fn sub_mcp_command(root: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sub-mcp"));
    command.env("SUB_CONFIG", root.join("absent-sub.toml"));
    command
}

/// Write an executable fixture from a child shell, so this multi-threaded test process never holds a write descriptor that a child forked by a sibling test could inherit; Linux fails `exec` of a file open for writing with `ETXTBSY`.
#[cfg(unix)]
fn write_executable(path: &std::path::Path, script: &str) {
    let status = Command::new("/bin/sh")
        .args([
            "-c",
            "printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"",
            "write-executable",
        ])
        .arg(path)
        .arg(script)
        .status()
        .unwrap_or_else(|error| panic!("write executable: {error}"));
    assert!(status.success(), "write executable: {status}");
}

fn existing_binary() -> std::path::PathBuf {
    std::env::current_exe().unwrap_or_else(|error| panic!("current executable: {error}"))
}

#[allow(clippy::needless_pass_by_value)]
fn rpc_call(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    request: serde_json::Value,
) -> serde_json::Value {
    let request = request.to_string();
    writeln!(stdin, "{request}").unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    let mut line = String::new();
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    serde_json::from_str(&line).unwrap_or_else(|error| panic!("response json: {error}"))
}

fn prepare_orphaned_task(root: &std::path::Path, handle: &str) {
    let attempt = root.join("tasks").join(handle).join("attempts/1");
    std::fs::create_dir_all(&attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let params = serde_json::json!({
        "harness": "codex",
        "prompt": "resume the bounded probe",
        "cwd": root,
        "harness_binary": "/bin/true",
        "model": null,
        "permission_mode": "agent"
    });
    let state = serde_json::json!({
        "number": 1,
        "status": "running",
        "supervisor_pid": u32::MAX,
        "supervisor_start_time": 1,
        "harness_session_id": "fixture-session",
        "usage": {"cost": null, "tokens": null}
    });
    std::fs::write(
        attempt.join("state.json"),
        serde_json::to_vec(&state).unwrap_or_else(|error| panic!("state json: {error}")),
    )
    .unwrap_or_else(|error| panic!("state: {error}"));
    std::fs::write(
        root.join("tasks").join(handle).join("task.json"),
        serde_json::to_vec(&serde_json::json!({
            "handle": {"id": handle},
            "params": params,
            "attempt": state
        }))
        .unwrap_or_else(|error| panic!("task json: {error}")),
    )
    .unwrap_or_else(|error| panic!("task: {error}"));
    std::fs::write(
        attempt.join("request.json"),
        serde_json::to_vec(&serde_json::json!({
            "params": params,
            "adapter": {
                "bridge": {"command": "/bin/false", "args": [], "env": {}},
                "session_meta": {},
                "delegation_guard": "Do not use subagents.",
                "resume_mechanism": "resume"
            },
            "resume_session_id": null
        }))
        .unwrap_or_else(|error| panic!("request json: {error}")),
    )
    .unwrap_or_else(|error| panic!("request: {error}"));
}
fn record_harness_child(root: &std::path::Path, handle: &str, pid: u32) {
    let state_path = root
        .join("tasks")
        .join(handle)
        .join("attempts/1/state.json");
    let mut state: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&state_path).unwrap_or_else(|error| panic!("read state: {error}")),
    )
    .unwrap_or_else(|error| panic!("state json: {error}"));
    let child =
        sub_sdk::delegation::HarnessChild::observe(pid).unwrap_or_else(|| panic!("child identity"));
    state["harness_child"] =
        serde_json::to_value(child).unwrap_or_else(|error| panic!("child json: {error}"));
    std::fs::write(
        &state_path,
        serde_json::to_vec(&state).unwrap_or_else(|error| panic!("state json: {error}")),
    )
    .unwrap_or_else(|error| panic!("state: {error}"));
}
#[path = "binary/cancel_ends_an_orphaned_task_over_stdio.rs"]
mod cancel_ends_an_orphaned_task_over_stdio;
#[path = "binary/configured_launch_values_flow_through_mcp.rs"]
mod configured_launch_values_flow_through_mcp;
#[path = "binary/prints_version.rs"]
mod prints_version;
#[path = "binary/recover_and_orphaned_wait_match_the_cli_over_stdio.rs"]
mod recover_and_orphaned_wait_match_the_cli_over_stdio;
#[path = "binary/serves_initialize_and_tool_list_over_stdio.rs"]
mod serves_initialize_and_tool_list_over_stdio;
#[path = "binary/supervisor_mode_rejects_missing_handle.rs"]
mod supervisor_mode_rejects_missing_handle;
#[path = "binary/tools_install_launch_and_wait_over_stdio.rs"]
mod tools_install_launch_and_wait_over_stdio;
