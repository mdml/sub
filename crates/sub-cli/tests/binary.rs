//! Smoke test: the `sub` binary runs and reports its version.

use std::process::Command;

#[cfg(unix)]
fn fake_npm(root: &std::path::Path) -> String {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    let npm = root.join("npm");
    fs::write(&npm, "#!/bin/sh\nwhile [ \"$1\" != \"--prefix\" ]; do shift; done\nshift\nprefix=$1\nmkdir -p \"$prefix/node_modules/.bin\"\nfor name in codex-acp claude-agent-acp; do printf '#!/bin/sh\\nexit 1\\n' > \"$prefix/node_modules/.bin/$name\"; chmod +x \"$prefix/node_modules/.bin/$name\"; done\n")
        .unwrap_or_else(|error| panic!("npm: {error}"));
    fs::set_permissions(&npm, fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("permissions: {error}"));
    format!(
        "{}:{}",
        root.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

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

    let cancel = Command::new(binary)
        .args(["cancel", "tsk_000000000000000000000000", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("run: {error}"));
    assert!(!cancel.status.success());
    assert!(String::from_utf8_lossy(&cancel.stderr).contains("unknown task handle"));

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
fn wait_reports_orphaned_and_recover_starts_the_next_attempt() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let handle = "tsk_454545454545454545454545";
    prepare_orphaned_task(root.path(), handle);
    let binary = env!("CARGO_BIN_EXE_sub");

    let wait = Command::new(binary)
        .args(["wait", handle, "--timeout-seconds", "0", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("wait: {error}"));
    assert!(wait.status.success());
    let wait: serde_json::Value =
        serde_json::from_slice(&wait.stdout).unwrap_or_else(|error| panic!("wait json: {error}"));
    assert_eq!(wait["state"], "orphaned");
    assert_eq!(wait["status"], "orphaned");

    let cancel = Command::new(binary)
        .args(["cancel", handle, "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("cancel: {error}"));
    assert!(cancel.status.success());
    let cancel: serde_json::Value = serde_json::from_slice(&cancel.stdout)
        .unwrap_or_else(|error| panic!("cancel json: {error}"));
    assert_eq!(cancel["handle"]["id"], handle);
    assert_eq!(cancel["attempt"], 1);
    assert_eq!(cancel["delivery"], "attempt_orphaned");

    let recover = Command::new(binary)
        .args(["recover", handle, "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("recover: {error}"));
    assert!(
        recover.status.success(),
        "{}",
        String::from_utf8_lossy(&recover.stderr)
    );
    let recovered: serde_json::Value = serde_json::from_slice(&recover.stdout)
        .unwrap_or_else(|error| panic!("recover json: {error}"));
    assert_eq!(recovered["handle"]["id"], handle);
    assert_eq!(recovered["attempt"], 2);

    let complete = Command::new(binary)
        .args(["wait", handle, "--timeout-seconds", "3", "--state-dir"])
        .arg(root.path())
        .output()
        .unwrap_or_else(|error| panic!("complete wait: {error}"));
    assert!(complete.status.success());
    let complete: serde_json::Value = serde_json::from_slice(&complete.stdout)
        .unwrap_or_else(|error| panic!("complete json: {error}"));
    assert_eq!(complete["state"], "complete");
    assert_eq!(complete["result"]["status"], "failed");
    assert!(
        complete["result"]["summary"]
            .as_str()
            .is_some_and(|summary| !summary.is_empty())
    );
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
        command.args(["--state-dir"]).arg(root.path());
        let launch = command
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

    for (extra, expected_binary, expected_model, expected_mode) in [
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
        let launch = Command::new(binary)
            .args(["launch", "--harness", "codex", "--cwd"])
            .arg(root.path())
            .args(["--prompt", "bounded config probe"])
            .args(extra)
            .env("SUB_CONFIG", &config)
            .output()
            .unwrap_or_else(|error| panic!("launch: {error}"));
        assert!(
            launch.status.success(),
            "{}",
            String::from_utf8_lossy(&launch.stderr)
        );
        let handle: serde_json::Value = serde_json::from_slice(&launch.stdout)
            .unwrap_or_else(|error| panic!("launch json: {error}"));
        let task = state
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
}

#[cfg(unix)]
#[test]
fn onboarding_is_scoped_and_idempotent_in_throwaway_roots() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let config = root.path().join("sub.toml");
    let state = root.path().join("state");
    std::fs::write(
        &config,
        format!(
            "state_dir = '{}'\n[harnesses.claude]\nbinary = '/bin/true'\npermission_mode = 'bypassPermissions'\n[harnesses.codex]\nbinary = '/bin/true'\npermission_mode = 'agent'\n",
            state.display()
        ),
    )
    .unwrap_or_else(|error| panic!("config: {error}"));
    let claude_config = root.path().join("claude/config.json");
    let claude_skills = root.path().join("claude/skills");
    let codex_config = root.path().join("codex/config.toml");
    let codex_skills = root.path().join("codex/skills");
    let binary = env!("CARGO_BIN_EXE_sub");
    let path = fake_npm(root.path());
    let run = |harnesses: &[&str]| {
        Command::new(binary)
            .arg("onboard")
            .args(harnesses)
            .env("SUB_CONFIG", &config)
            .env("SUB_CLAUDE_CONFIG", &claude_config)
            .env("SUB_CLAUDE_SKILLS_DIR", &claude_skills)
            .env("SUB_CODEX_CONFIG", &codex_config)
            .env("SUB_CODEX_SKILLS_DIR", &codex_skills)
            .env("SUB_MCP_BINARY", "/bin/true")
            .env("PATH", &path)
            .output()
            .unwrap_or_else(|error| panic!("onboard: {error}"))
    };

    let claude_only = run(&["claude"]);
    assert!(claude_only.status.success());
    assert!(claude_config.is_file());
    assert!(claude_skills.join("sub-delegation/SKILL.md").is_file());
    assert!(!codex_config.exists());
    assert!(!codex_skills.exists());

    let first_codex = run(&["codex"]);
    assert!(first_codex.status.success());
    let second = run(&["claude", "codex"]);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&second.stdout).unwrap_or_else(|error| panic!("report: {error}"));
    for harness in report.as_array().unwrap_or_else(|| panic!("array")) {
        assert_eq!(harness["bridge"]["status"], "unchanged");
        assert_eq!(harness["skill"]["status"], "unchanged");
        assert_eq!(harness["mcp"]["status"], "unchanged");
    }
    assert!(codex_skills.join("sub-delegation/SKILL.md").is_file());
    assert!(
        std::fs::read_to_string(codex_config)
            .unwrap_or_else(|error| panic!("codex config: {error}"))
            .contains("[mcp_servers.sub]")
    );
}
