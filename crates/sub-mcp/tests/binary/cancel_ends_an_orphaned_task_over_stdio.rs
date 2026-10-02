use super::*;

#[cfg(unix)]
#[test]
fn cancel_ends_an_orphaned_task_over_stdio() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = "tsk_575757575757575757575757";
    let mut orphan = Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn child: {error}"));
    prepare_orphaned_task(root.path(), handle);
    record_harness_child(root.path(), handle, orphan.id());
    let mut child = sub_mcp_command(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    let mut stdin = child.stdin.take().unwrap_or_else(|| panic!("stdin"));
    let mut stdout = BufReader::new(child.stdout.take().unwrap_or_else(|| panic!("stdout")));
    let state = root.path().to_string_lossy();

    let cancelled = rpc_call(
        &mut stdin,
        &mut stdout,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sub_cancel","arguments":{"handle":handle,"state_dir":state}}}),
    );
    assert_eq!(
        cancelled["result"]["structuredContent"]["delivery"],
        "attempt_orphaned"
    );
    assert!(
        !orphan
            .wait()
            .unwrap_or_else(|error| panic!("orphan wait: {error}"))
            .success()
    );

    let complete = rpc_call(
        &mut stdin,
        &mut stdout,
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"sub_wait","arguments":{"handle":handle,"timeout_seconds":0,"state_dir":state}}}),
    );
    assert_eq!(complete["result"]["structuredContent"]["state"], "complete");
    assert_eq!(
        complete["result"]["structuredContent"]["result"]["status"],
        "cancelled"
    );

    let inspected = rpc_call(
        &mut stdin,
        &mut stdout,
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"sub_inspect","arguments":{"handle":handle,"state_dir":state}}}),
    );
    let events = &inspected["result"]["structuredContent"]["events"];
    assert_eq!(events[1]["kind"], "orphaned_child_disposed");
    assert_eq!(events[1]["disposition"], "terminated");
    assert_eq!(events[3]["kind"], "attempt_finished");
    assert_eq!(events[3]["status"], "cancelled");

    let recovered = rpc_call(
        &mut stdin,
        &mut stdout,
        serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"sub_recover","arguments":{"handle":handle,"state_dir":state}}}),
    );
    assert_eq!(recovered["result"]["isError"], true, "{recovered}");
    drop(stdin);
    assert!(
        child
            .wait()
            .unwrap_or_else(|error| panic!("wait: {error}"))
            .success()
    );
}
