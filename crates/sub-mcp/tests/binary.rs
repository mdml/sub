//! Smoke test: the `sub-mcp` binary runs and reports its version.

use std::io::Write;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

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
