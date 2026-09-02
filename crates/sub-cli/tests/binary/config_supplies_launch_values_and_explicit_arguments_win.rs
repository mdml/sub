use super::*;

#[cfg(unix)]
#[test]
fn config_supplies_launch_values_and_explicit_arguments_win() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let state = root.path().join("state");
    let config = root.path().join("sub.toml");
    std::fs::write(
        &config,
        format!(
            "state_dir = '{}'\n[harnesses.codex]\nbinary = '/bin/true'\nmodel = 'configured-model'\npermission_mode = 'configured-mode'\n",
            state.display()
        ),
    )
    .unwrap_or_else(|error| panic!("config: {error}"));
    let binary = env!("CARGO_BIN_EXE_sub");
    let path = fake_npm(root.path());
    let install = Command::new(binary)
        .args(["bridge", "install", "codex"])
        .env("SUB_CONFIG", &config)
        .env("PATH", &path)
        .output()
        .unwrap_or_else(|error| panic!("install: {error}"));
    assert!(install.status.success());

    let context = LaunchContext {
        root: root.path(),
        state: &state,
        config: &config,
        binary,
    };
    for expected in [
        (
            Vec::<&str>::new(),
            "/bin/true",
            "configured-model",
            "configured-mode",
        ),
        (
            vec![
                "--binary",
                "/bin/false",
                "--model",
                "explicit-model",
                "--permission-mode",
                "explicit-mode",
            ],
            "/bin/false",
            "explicit-model",
            "explicit-mode",
        ),
    ] {
        assert_launch_config(&context, expected);
    }
}

struct LaunchContext<'a> {
    root: &'a std::path::Path,
    state: &'a std::path::Path,
    config: &'a std::path::Path,
    binary: &'a str,
}

fn assert_launch_config(context: &LaunchContext<'_>, expected: (Vec<&str>, &str, &str, &str)) {
    let (extra, expected_binary, expected_model, expected_mode) = expected;
    let launch = Command::new(context.binary)
        .args(["launch", "--harness", "codex", "--cwd"])
        .arg(context.root)
        .args(["--prompt", "bounded config probe"])
        .args(extra)
        .env("SUB_CONFIG", context.config)
        .output()
        .unwrap_or_else(|error| panic!("launch: {error}"));
    assert!(
        launch.status.success(),
        "{}",
        String::from_utf8_lossy(&launch.stderr)
    );
    let handle: serde_json::Value = serde_json::from_slice(&launch.stdout)
        .unwrap_or_else(|error| panic!("launch json: {error}"));
    let task = context
        .state
        .join("tasks")
        .join(handle["id"].as_str().unwrap_or_else(|| panic!("handle")))
        .join("task.json");
    let persisted: serde_json::Value = serde_json::from_slice(
        &std::fs::read(task).unwrap_or_else(|error| panic!("task: {error}")),
    )
    .unwrap_or_else(|error| panic!("task json: {error}"));
    assert_eq!(persisted["params"]["harness_binary"], expected_binary);
    assert_eq!(persisted["params"]["model"], expected_model);
    assert_eq!(persisted["params"]["permission_mode"], expected_mode);
}
