use super::*;

#[cfg(unix)]
#[test]
fn configured_launch_values_flow_through_mcp() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let state = root.path().join("state");
    let config = root.path().join("sub.toml");
    let harness_binary = existing_binary().to_string_lossy().into_owned();
    fs::write(
        &config,
        format!(
            "state_dir = '{}'\n[harnesses.codex]\nbinary = '{}'\nmodel = 'mcp-config-model'\npermission_mode = 'agent'\n",
            state.display(),
            harness_binary
        ),
    )
    .unwrap_or_else(|error| panic!("config: {error}"));
    let npm = root.path().join("npm");
    fs::write(&npm, "#!/bin/sh\nwhile [ \"$1\" != \"--prefix\" ]; do shift; done\nshift\nprefix=$1\nmkdir -p \"$prefix/node_modules/.bin\"\nprintf '#!/bin/sh\\nexit 1\\n' > \"$prefix/node_modules/.bin/codex-acp\"\nchmod +x \"$prefix/node_modules/.bin/codex-acp\"\n")
        .unwrap_or_else(|error| panic!("npm: {error}"));
    fs::set_permissions(&npm, fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("permissions: {error}"));
    let path = format!(
        "{}:{}",
        root.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_sub-mcp"))
        .env("SUB_CONFIG", &config)
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    let mut stdin = child.stdin.take().unwrap_or_else(|| panic!("stdin"));
    let mut stdout = BufReader::new(child.stdout.take().unwrap_or_else(|| panic!("stdout")));
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sub_bridge_install","arguments":{"harness":"codex"}}}))
        .unwrap_or_else(|error| panic!("write: {error}"));
    writeln!(stdin, "{}", serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"sub_launch","arguments":{"harness":"codex","prompt":"configured MCP probe","cwd":root.path()}}}))
        .unwrap_or_else(|error| panic!("write: {error}"));
    drop(stdin);
    let mut install_line = String::new();
    stdout
        .read_line(&mut install_line)
        .unwrap_or_else(|error| panic!("read install: {error}"));
    assert!(install_line.contains("bridge_binary"));
    let mut launch_line = String::new();
    stdout
        .read_line(&mut launch_line)
        .unwrap_or_else(|error| panic!("read launch: {error}"));
    let launch: serde_json::Value =
        serde_json::from_str(&launch_line).unwrap_or_else(|error| panic!("launch json: {error}"));
    let handle = launch["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("handle"));
    let persisted: serde_json::Value = serde_json::from_slice(
        &fs::read(state.join("tasks").join(handle).join("task.json"))
            .unwrap_or_else(|error| panic!("task: {error}")),
    )
    .unwrap_or_else(|error| panic!("task json: {error}"));
    assert_eq!(persisted["params"]["harness_binary"], harness_binary);
    assert_eq!(persisted["params"]["model"], "mcp-config-model");
    assert_eq!(persisted["params"]["permission_mode"], "agent");
    assert!(
        child
            .wait()
            .unwrap_or_else(|error| panic!("wait: {error}"))
            .success()
    );
}
