use super::*;

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

    let waited = rpc_call(
        &mut stdin,
        &mut stdout,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sub_wait","arguments":{"handle":handle,"timeout_seconds":0,"state_dir":state}}}),
    );
    assert_eq!(waited["result"]["structuredContent"]["state"], "orphaned");

    let cancelled = rpc_call(
        &mut stdin,
        &mut stdout,
        serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"sub_cancel","arguments":{"handle":handle,"state_dir":state}}}),
    );
    assert_eq!(
        cancelled["result"]["structuredContent"]["delivery"],
        "attempt_orphaned"
    );

    let recovered = rpc_call(
        &mut stdin,
        &mut stdout,
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"sub_recover","arguments":{"handle":handle,"state_dir":state}}}),
    );
    assert_eq!(
        recovered["result"]["structuredContent"]["handle"]["id"],
        handle
    );
    assert_eq!(recovered["result"]["structuredContent"]["attempt"], 2);

    let complete = rpc_call(
        &mut stdin,
        &mut stdout,
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"sub_wait","arguments":{"handle":handle,"timeout_seconds":3,"state_dir":state}}}),
    );
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
