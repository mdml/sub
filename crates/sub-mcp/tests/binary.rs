//! Smoke test: the `sub-mcp` binary runs and reports its version.

use std::io::Write;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

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

#[test]
fn prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_sub-mcp"))
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("failed to run sub-mcp: {e}"));
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        format!("sub-mcp {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn serves_initialize_and_tool_list_over_stdio() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sub-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    let mut stdin = child.stdin.take().unwrap_or_else(|| panic!("stdin"));
    stdin.write_all(b"\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n")
        .unwrap_or_else(|error| panic!("write: {error}"));
    drop(stdin);
    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("wait: {error}"));
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sub_launch"));
    assert!(stdout.contains("sub_wait"));
    assert!(stdout.contains("sub_recover"));
}

#[test]
fn supervisor_mode_rejects_missing_handle() {
    let output = Command::new(env!("CARGO_BIN_EXE_sub-mcp"))
        .arg("__supervise")
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("handle missing"));
}

#[cfg(unix)]
#[test]
fn recover_and_orphaned_wait_match_the_cli_over_stdio() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = "tsk_565656565656565656565656";
    prepare_orphaned_task(root.path(), handle);
    let mut child = Command::new(env!("CARGO_BIN_EXE_sub-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    let mut stdin = child.stdin.take().unwrap_or_else(|| panic!("stdin"));
    let mut stdout = BufReader::new(child.stdout.take().unwrap_or_else(|| panic!("stdout")));
    let state = root.path().to_string_lossy();

    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sub_wait","arguments":{"handle":handle,"timeout_seconds":0,"state_dir":state}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    let mut line = String::new();
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    let waited: serde_json::Value =
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("wait json: {error}"));
    assert_eq!(waited["result"]["structuredContent"]["state"], "orphaned");

    line.clear();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"sub_cancel","arguments":{"handle":handle,"state_dir":state}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    let cancelled: serde_json::Value =
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("cancel json: {error}"));
    assert_eq!(
        cancelled["result"]["structuredContent"]["delivery"],
        "attempt_orphaned"
    );

    line.clear();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"sub_recover","arguments":{"handle":handle,"state_dir":state}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    let recovered: serde_json::Value =
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("recover json: {error}"));
    assert_eq!(
        recovered["result"]["structuredContent"]["handle"]["id"],
        handle
    );
    assert_eq!(recovered["result"]["structuredContent"]["attempt"], 2);

    line.clear();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"sub_wait","arguments":{"handle":handle,"timeout_seconds":3,"state_dir":state}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    let complete: serde_json::Value =
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("complete json: {error}"));
    assert_eq!(complete["result"]["structuredContent"]["state"], "complete");
    assert_eq!(
        complete["result"]["structuredContent"]["result"]["status"],
        "failed"
    );
    drop(stdin);
    assert!(
        child
            .wait()
            .unwrap_or_else(|error| panic!("wait: {error}"))
            .success()
    );
}

#[cfg(unix)]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one stdio session verifies all matching MCP task controls"
)]
fn tools_install_launch_and_wait_over_stdio() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let npm = root.path().join("npm");
    fs::write(&npm, "#!/bin/sh\nwhile [ \"$1\" != \"--prefix\" ]; do shift; done\nshift\nprefix=$1\nmkdir -p \"$prefix/node_modules/.bin\"\nfor name in codex-acp claude-agent-acp; do printf '#!/bin/sh\\nexit 1\\n' > \"$prefix/node_modules/.bin/$name\"; chmod +x \"$prefix/node_modules/.bin/$name\"; done\n")
        .unwrap_or_else(|error| panic!("npm: {error}"));
    fs::set_permissions(&npm, fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("permissions: {error}"));
    let path = format!(
        "{}:{}",
        root.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_sub-mcp"))
        .env("PATH", path)
        .env("SUB_STATE_DIR", root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    let mut stdin = child.stdin.take().unwrap_or_else(|| panic!("stdin"));
    let mut stdout = BufReader::new(child.stdout.take().unwrap_or_else(|| panic!("stdout")));
    let state = root.path().to_string_lossy();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sub_bridge_install","arguments":{"harness":"codex","state_dir":state}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    let mut line = String::new();
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    assert!(line.contains("bridge_binary"));
    line.clear();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"sub_bridge_install","arguments":{"harness":"claude"}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    assert!(line.contains("claude-agent-acp"));
    line.clear();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"sub_launch","arguments":{"harness":"codex","prompt":"probe","cwd":state,"binary":"/bin/true","permission_mode":"agent","model":"test","state_dir":state}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    let launch: serde_json::Value =
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("launch json: {error}"));
    let handle = launch["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("handle"))
        .to_owned();
    line.clear();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"sub_wait","arguments":{"handle":handle,"timeout_seconds":3,"state_dir":state}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    assert!(line.contains("failed"));
    line.clear();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"sub_list","arguments":{"state_dir":state}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    assert!(line.contains(&handle));
    line.clear();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"sub_inspect","arguments":{"handle":handle,"state_dir":state}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    let inspection: serde_json::Value =
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("inspect json: {error}"));
    assert_eq!(
        inspection["result"]["structuredContent"]["task"]["handle"]["id"],
        handle
    );
    line.clear();
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"sub_launch","arguments":{"harness":"claude","prompt":"probe","cwd":state,"binary":"/bin/true","permission_mode":"default"}}})).unwrap_or_else(|error| panic!("write: {error}"));
    stdin
        .flush()
        .unwrap_or_else(|error| panic!("flush: {error}"));
    stdout
        .read_line(&mut line)
        .unwrap_or_else(|error| panic!("read: {error}"));
    assert!(line.contains("tsk_"));
    drop(stdin);
    assert!(
        child
            .wait()
            .unwrap_or_else(|error| panic!("wait: {error}"))
            .success()
    );
}
