use super::*;

#[cfg(unix)]
#[test]
fn tools_install_launch_and_wait_over_stdio() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let mut child = spawn_test_mcp(root.path());
    let mut stdin = child.stdin.take().unwrap_or_else(|| panic!("stdin"));
    let mut stdout = BufReader::new(child.stdout.take().unwrap_or_else(|| panic!("stdout")));
    let state = root.path().to_string_lossy();
    let harness_binary = existing_binary().to_string_lossy().into_owned();
    assert_task_controls(&mut stdin, &mut stdout, &state, &harness_binary);
    drop(stdin);
    assert!(
        child
            .wait()
            .unwrap_or_else(|error| panic!("wait: {error}"))
            .success()
    );
}

fn spawn_test_mcp(root: &std::path::Path) -> std::process::Child {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    let npm = root.join("npm");
    fs::write(&npm, "#!/bin/sh\nwhile [ \"$1\" != \"--prefix\" ]; do shift; done\nshift\nprefix=$1\nmkdir -p \"$prefix/node_modules/.bin\"\nfor name in codex-acp claude-agent-acp; do printf '#!/bin/sh\\nexit 1\\n' > \"$prefix/node_modules/.bin/$name\"; chmod +x \"$prefix/node_modules/.bin/$name\"; done\n")
        .unwrap_or_else(|error| panic!("npm: {error}"));
    fs::set_permissions(&npm, fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("permissions: {error}"));
    let path = format!(
        "{}:{}",
        root.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new(env!("CARGO_BIN_EXE_sub-mcp"))
        .env("PATH", path)
        .env("SUB_STATE_DIR", root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn: {error}"))
}

fn assert_task_controls(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    state: &str,
    harness_binary: &str,
) {
    let installed = rpc_call(
        stdin,
        stdout,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sub_bridge_install","arguments":{"harness":"codex","state_dir":state}}}),
    );
    assert!(installed.to_string().contains("bridge_binary"));
    let claude = rpc_call(
        stdin,
        stdout,
        serde_json::json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"sub_bridge_install","arguments":{"harness":"claude"}}}),
    );
    assert!(claude.to_string().contains("claude-agent-acp"));
    let launch = rpc_call(
        stdin,
        stdout,
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"sub_launch","arguments":{"harness":"codex","prompt":"probe","cwd":state,"binary":harness_binary,"permission_mode":"agent","model":"test","state_dir":state}}}),
    );
    let handle = launch["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("handle"))
        .to_owned();
    let waited = rpc_call(
        stdin,
        stdout,
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"sub_wait","arguments":{"handle":handle,"timeout_seconds":3,"state_dir":state}}}),
    );
    assert!(waited.to_string().contains("failed"));
    let listed = rpc_call(
        stdin,
        stdout,
        serde_json::json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"sub_list","arguments":{"state_dir":state}}}),
    );
    assert!(listed.to_string().contains(&handle));
    let inspection = rpc_call(
        stdin,
        stdout,
        serde_json::json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"sub_inspect","arguments":{"handle":handle,"state_dir":state}}}),
    );
    assert_eq!(
        inspection["result"]["structuredContent"]["task"]["handle"]["id"],
        handle
    );
    let claude_launch = rpc_call(
        stdin,
        stdout,
        serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"sub_launch","arguments":{"harness":"claude","prompt":"probe","cwd":state,"binary":harness_binary,"permission_mode":"default"}}}),
    );
    assert!(claude_launch.to_string().contains("tsk_"));
}
