use super::*;

#[cfg(unix)]
#[test]
fn install_launch_and_wait_use_one_durable_shape() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let path = fake_npm(root.path());
    let binary = env!("CARGO_BIN_EXE_sub");
    for harness in ["claude", "codex"] {
        let output = sub_command(binary, root.path())
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
    let mut command = sub_command(binary, root);
    command
        .args(["launch", "--harness", harness.name(), "--cwd"])
        .arg(root)
        .args(["--prompt", "bounded probe", "--binary"])
        .arg(binary)
        .args(["--permission-mode", harness.permission()]);
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
    let wait = sub_command(binary, root)
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
    let listed = sub_command(binary, root)
        .args(["list", "--state-dir"])
        .arg(root)
        .output()
        .unwrap_or_else(|error| panic!("list: {error}"));
    assert!(listed.status.success());
    assert!(String::from_utf8_lossy(&listed.stdout).contains(handle));
    let inspected = sub_command(binary, root)
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
