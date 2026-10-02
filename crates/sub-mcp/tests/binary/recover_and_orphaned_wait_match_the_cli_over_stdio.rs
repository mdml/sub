use super::*;

/// One `sub-mcp` stdio session that numbers its own JSON-RPC requests.
struct Session {
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u32,
}

impl Session {
    /// Call one `sub` tool and return its structured content.
    fn call(&mut self, name: &str, arguments: &serde_json::Value) -> serde_json::Value {
        self.next_id += 1;
        let response = rpc_call(
            &mut self.stdin,
            &mut self.stdout,
            serde_json::json!({"jsonrpc":"2.0","id":self.next_id,"method":"tools/call","params":{"name":name,"arguments":arguments}}),
        );
        response["result"]["structuredContent"].clone()
    }
}

#[cfg(unix)]
#[test]
fn recover_and_orphaned_wait_match_the_cli_over_stdio() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = "tsk_565656565656565656565656";
    let mut orphan = Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn orphan: {error}"));
    prepare_orphaned_task(root.path(), handle);
    record_harness_child(root.path(), handle, orphan.id());
    let mut child = sub_mcp_command(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    let mut session = Session {
        stdin: child.stdin.take().unwrap_or_else(|| panic!("stdin")),
        stdout: BufReader::new(child.stdout.take().unwrap_or_else(|| panic!("stdout"))),
        next_id: 0,
    };
    let state = root.path().to_string_lossy();
    let arguments = serde_json::json!({"handle": handle, "state_dir": state});

    let waited = session.call(
        "sub_wait",
        &serde_json::json!({"handle": handle, "timeout_seconds": 0, "state_dir": state}),
    );
    assert_eq!(waited["state"], "orphaned");

    let recovered = session.call("sub_recover", &arguments);
    assert_eq!(recovered["handle"]["id"], handle);
    assert_eq!(recovered["attempt"], 2);
    assert!(
        !orphan
            .wait()
            .unwrap_or_else(|error| panic!("orphan wait: {error}"))
            .success(),
        "recover must end the orphaned attempt's child"
    );

    let inspected = session.call("sub_inspect", &arguments);
    assert_eq!(inspected["events"][1]["kind"], "orphaned_child_disposed");
    assert_eq!(inspected["events"][1]["disposition"], "terminated");

    let complete = session.call(
        "sub_wait",
        &serde_json::json!({"handle": handle, "timeout_seconds": 3, "state_dir": state}),
    );
    assert_eq!(complete["state"], "complete");
    assert_eq!(complete["result"]["status"], "failed");

    let cancelled = session.call("sub_cancel", &arguments);
    assert_eq!(cancelled["delivery"], "already_finished");
    drop(session);
    assert!(
        child
            .wait()
            .unwrap_or_else(|error| panic!("wait: {error}"))
            .success()
    );
}
