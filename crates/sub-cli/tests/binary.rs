//! Smoke test: the `sub` binary runs and reports its version.

use std::process::Command;

fn existing_binary() -> std::path::PathBuf {
    std::env::current_exe().unwrap_or_else(|error| panic!("current executable: {error}"))
}

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
#[path = "binary/command_errors_are_actionable.rs"]
mod command_errors_are_actionable;
#[path = "binary/config_supplies_launch_values_and_explicit_arguments_win.rs"]
mod config_supplies_launch_values_and_explicit_arguments_win;
#[path = "binary/install_launch_and_wait_use_one_durable_shape.rs"]
mod install_launch_and_wait_use_one_durable_shape;
#[path = "binary/onboarding_is_scoped_and_idempotent_in_throwaway_roots.rs"]
mod onboarding_is_scoped_and_idempotent_in_throwaway_roots;
#[path = "binary/prints_version.rs"]
mod prints_version;
#[path = "binary/wait_reports_orphaned_and_recover_starts_the_next_attempt.rs"]
mod wait_reports_orphaned_and_recover_starts_the_next_attempt;
