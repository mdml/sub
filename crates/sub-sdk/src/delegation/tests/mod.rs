use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::acp::{StopReason, StreamUpdate, StreamUpdateKind};

use super::liveness::process_start_time;
use super::result::{derive_changed_files, markdown_destinations};
use super::supervisor::supervisor_command;
use super::*;

fn fake_binary() -> PathBuf {
    let executable = std::env::current_exe().unwrap_or_else(|error| panic!("exe: {error}"));
    let debug = executable
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| panic!("debug dir"));
    let direct = debug.join("sub-harness-fake");
    if direct.is_file() {
        return direct;
    }
    let sibling = debug
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| panic!("target dir"))
        .join("debug/sub-harness-fake");
    if sibling.is_file() {
        return sibling;
    }
    panic!("sub-harness-fake binary not found");
}

fn fake_request(root: &Path, scenario: &str) -> SupervisorRequest {
    SupervisorRequest {
        params: LaunchParams {
            harness: Harness::Codex,
            prompt: "contract probe".to_owned(),
            cwd: root.to_path_buf(),
            harness_binary: fake_binary(),
            model: Some("fixture-model".to_owned()),
            permission_mode: "agent".to_owned(),
        },
        adapter: AdapterLaunch {
            bridge: HarnessLaunch::new(fake_binary())
                .arg(scenario)
                .env(
                    "SUB_FAKE_FIXTURES_DIR",
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../sub-harness-fake/fixtures")
                        .to_string_lossy()
                        .into_owned(),
                )
                .env(
                    "SUB_FAKE_SCENARIOS_DIR",
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../sub-harness-fake/scenarios")
                        .to_string_lossy()
                        .into_owned(),
                ),
            session_meta: serde_json::json!({}),
            delegation_guard: "Do not use subagents.".to_owned(),
            resume_mechanism: ResumeMechanism::Resume,
        },
        resume_session_id: None,
    }
}

fn prepare_supervisor(root: &Path, handle: &TaskHandle, request: &SupervisorRequest) -> TaskPaths {
    let paths = TaskPaths::new(root, handle);
    fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
    write_json(&paths.request, request).unwrap_or_else(|error| panic!("request: {error}"));
    paths
}

fn prepare_resume_attempt(
    root: &Path,
    handle: &TaskHandle,
    mut request: SupervisorRequest,
    session_id: Option<&str>,
) -> TaskPaths {
    let paths = TaskPaths::for_attempt(root, handle, 2);
    fs::create_dir_all(&paths.attempt).unwrap_or_else(|error| panic!("mkdir: {error}"));
    request.resume_session_id = session_id.map(str::to_owned);
    write_json(&paths.request, &request).unwrap_or_else(|error| panic!("request: {error}"));
    paths
}

mod lifecycle_cancel_orphan;
mod lifecycle_cancel_terminal;
mod lifecycle_list;
mod lifecycle_orphan;
mod lifecycle_recovery;
mod lifecycle_wait;
mod observation;
mod result;
mod state;
mod supervisor_cancel_honored;
mod supervisor_cancel_ignored;
mod supervisor_failure;
mod supervisor_permission;
mod supervisor_resume_missing;
mod supervisor_resume_refused;
mod supervisor_success;
