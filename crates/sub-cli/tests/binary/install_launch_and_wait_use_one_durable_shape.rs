use super::*;

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
    for harness in [HarnessCase::Codex, HarnessCase::Claude] {
        exercise_harness(root.path(), std::path::Path::new(binary), harness);
    }
}

#[derive(Clone, Copy)]
enum HarnessCase {
    Claude,
    Codex,
}

impl HarnessCase {
    const fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    const fn permission(self) -> &'static str {
        match self {
            Self::Claude => "default",
            Self::Codex => "agent",
        }
    }
}

fn exercise_harness(root: &std::path::Path, binary: &std::path::Path, harness: HarnessCase) {
    let mut command = Command::new(binary);
    command
        .args(["launch", "--harness", harness.name(), "--cwd"])
        .arg(root)
        .args([
            "--prompt",
            "bounded probe",
            "--binary",
            "/bin/true",
            "--permission-mode",
            harness.permission(),
        ]);
    if matches!(harness, HarnessCase::Codex) {
        command.args(["--model", "test"]);
    }
    let launch = command
        .args(["--state-dir"])
        .arg(root)
        .env_remove("SUB_CONFIG")
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("HOME")
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
        .env("SUB_STATE_DIR", root)
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
        .arg(root)
        .output()
        .unwrap_or_else(|error| panic!("list: {error}"));
    assert!(listed.status.success());
    assert!(String::from_utf8_lossy(&listed.stdout).contains(handle));
    let inspected = Command::new(binary)
        .args(["inspect", handle, "--state-dir"])
        .arg(root)
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
