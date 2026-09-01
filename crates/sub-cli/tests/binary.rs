//! Smoke test: the `sub` binary runs and reports its version.

use std::process::Command;

#[test]
fn prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_sub"))
        .output()
        .unwrap_or_else(|e| panic!("failed to run sub: {e}"));
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), format!("sub {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn command_errors_are_actionable() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let binary = env!("CARGO_BIN_EXE_sub");
    let unsupported = Command::new(binary)
        .args(["bridge", "install", "cursor", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!unsupported.status.success());
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("unsupported harness"));

    let unknown = Command::new(binary)
        .args([
            "wait",
            "tsk_000000000000000000000000",
            "--timeout-seconds",
            "0",
            "--state-dir",
        ])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown task handle"));

    let recover = Command::new(binary)
        .args(["recover", "tsk_000000000000000000000000", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!recover.status.success());
    assert!(String::from_utf8_lossy(&recover.stderr).contains("unknown task handle"));

    let incomplete = Command::new(binary)
        .arg("launch")
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!incomplete.status.success());
    assert!(String::from_utf8_lossy(&incomplete.stderr).contains("--harness is required"));

    for args in [
        vec!["bridge", "install"],
        vec!["wait"],
        vec!["not-a-command"],
    ] {
        let output = Command::new(binary)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run: {error}"));
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("usage"));
    }
}

#[cfg(unix)]
#[test]
fn install_launch_and_wait_use_one_durable_shape() {
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
    let binary = env!("CARGO_BIN_EXE_sub");
    for harness in ["claude", "codex"] {
        let output = Command::new(binary)
            .args(["bridge", "install", harness, "--state-dir"])
            .arg(root.path())
            .env("PATH", &path)
            .output()
            .unwrap_or_else(|error| panic!("install: {error}"));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for harness in ["codex", "claude"] {
        let mut command = Command::new(binary);
        command
            .args(["launch", "--harness", harness, "--cwd"])
            .arg(root.path())
            .args([
                "--prompt",
                "bounded probe",
                "--binary",
                "/bin/true",
                "--permission-mode",
                if harness == "codex" {
                    "agent"
                } else {
                    "default"
                },
            ]);
        if harness == "codex" {
            command.args(["--model", "test"]);
        }
        let launch = command
            .env("SUB_STATE_DIR", root.path())
            .output()
            .unwrap_or_else(|error| panic!("launch: {error}"));
        assert!(
            launch.status.success(),
            "{}",
            String::from_utf8_lossy(&launch.stderr)
        );
        let value: serde_json::Value =
            serde_json::from_slice(&launch.stdout).unwrap_or_else(|error| panic!("json: {error}"));
        let handle = value["id"].as_str().unwrap_or_else(|| panic!("handle"));
        let wait = Command::new(binary)
            .args(["wait", handle, "--timeout-seconds", "3"])
            .env("SUB_STATE_DIR", root.path())
            .output()
            .unwrap_or_else(|error| panic!("wait: {error}"));
        assert!(
            wait.status.success(),
            "{}",
            String::from_utf8_lossy(&wait.stderr)
        );
        assert!(String::from_utf8_lossy(&wait.stdout).contains("failed"));
        let listed = Command::new(binary)
            .args(["list", "--state-dir"])
            .arg(root.path())
            .output()
            .unwrap_or_else(|error| panic!("list: {error}"));
        assert!(listed.status.success());
        assert!(String::from_utf8_lossy(&listed.stdout).contains(handle));
        let inspected = Command::new(binary)
            .args(["inspect", handle, "--state-dir"])
            .arg(root.path())
            .output()
            .unwrap_or_else(|error| panic!("inspect: {error}"));
        assert!(inspected.status.success());
        let inspection: serde_json::Value = serde_json::from_slice(&inspected.stdout)
            .unwrap_or_else(|error| panic!("inspect json: {error}"));
        assert_eq!(inspection["task"]["handle"]["id"], handle);
        assert!(
            inspection["task"]["usage_support"]["tokens"]
                .as_bool()
                .is_some()
        );
    }
}
