use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::acp::{StopReason, StreamUpdate, TurnUsage};

use super::state::TaskPaths;
use super::{ArtifactKind, ArtifactReference, Harness, TaskResult, TaskStatus};

pub(super) fn derive_task_result(
    outcome: Result<(crate::acp::SessionHandle, crate::acp::PromptResult), crate::acp::AcpError>,
    paths: &TaskPaths,
    harness: Harness,
    cwd: &Path,
) -> (TaskResult, Option<TurnUsage>) {
    match outcome {
        Ok((session, prompt_result)) => (
            TaskResult {
                status: status_from_stop(prompt_result.stop_reason),
                summary: prompt_result.final_text.trim().to_owned(),
                changed_files: derive_changed_files(
                    &prompt_result.updates,
                    &prompt_result.final_text,
                    cwd,
                ),
                artifacts: artifacts(paths, harness, cwd, &session.session_id),
                harness_session_id: Some(session.session_id),
            },
            prompt_result.usage,
        ),
        Err(error) => (
            TaskResult {
                status: TaskStatus::Failed,
                summary: error.to_string(),
                changed_files: Vec::new(),
                artifacts: base_artifacts(paths),
                harness_session_id: None,
            },
            None,
        ),
    }
}

pub(super) fn derive_changed_files(
    updates: &[StreamUpdate],
    final_text: &str,
    cwd: &Path,
) -> Vec<PathBuf> {
    let mut files = updates
        .iter()
        .flat_map(|update| update.changed_files.iter().cloned())
        .collect::<BTreeSet<_>>();
    for destination in markdown_destinations(final_text) {
        let path = PathBuf::from(destination);
        let absolute = if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        };
        if absolute.starts_with(cwd) && absolute.is_file() {
            files.insert(absolute);
        }
    }
    files.into_iter().collect()
}

pub(super) fn looks_like_subagent(update: &StreamUpdate) -> bool {
    if !matches!(
        update.kind,
        crate::acp::StreamUpdateKind::ToolCall | crate::acp::StreamUpdateKind::ToolCallUpdate
    ) {
        return false;
    }
    update.text.as_deref().is_some_and(|title| {
        let normalized = title.to_ascii_lowercase();
        normalized == "agent"
            || normalized == "task"
            || normalized.contains("subagent")
            || normalized.contains("spawn_agent")
    })
}

pub(super) fn markdown_destinations(text: &str) -> Vec<&str> {
    let mut destinations = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("](") {
        remaining = &remaining[start + 2..];
        let (candidate, rest) = if let Some(stripped) = remaining.strip_prefix('<') {
            let Some(end) = stripped.find(">)") else {
                break;
            };
            (&stripped[..end], &stripped[end + 2..])
        } else {
            let Some(end) = remaining.find(')') else {
                break;
            };
            (&remaining[..end], &remaining[end + 1..])
        };
        destinations.push(candidate);
        remaining = rest;
    }
    destinations
}

pub(super) fn status_from_stop(reason: StopReason) -> TaskStatus {
    match reason {
        StopReason::EndTurn => TaskStatus::Succeeded,
        StopReason::Cancelled => TaskStatus::Cancelled,
        StopReason::MaxTokens | StopReason::MaxTurnRequests | StopReason::Refusal => {
            TaskStatus::Failed
        }
    }
}

pub(super) fn base_artifacts(paths: &TaskPaths) -> Vec<ArtifactReference> {
    vec![
        ArtifactReference {
            kind: ArtifactKind::EventLog,
            location: paths.events.display().to_string(),
        },
        ArtifactReference {
            kind: ArtifactKind::SupervisorLog,
            location: paths.supervisor_log.display().to_string(),
        },
    ]
}

fn artifacts(
    paths: &TaskPaths,
    harness: Harness,
    cwd: &Path,
    session_id: &str,
) -> Vec<ArtifactReference> {
    let mut values = base_artifacts(paths);
    values.push(ArtifactReference {
        kind: ArtifactKind::NativeSession,
        location: native_session_reference(harness, cwd, session_id),
    });
    values
}

pub(super) fn native_session_reference(harness: Harness, cwd: &Path, session_id: &str) -> String {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return format!(
            "{}:{session_id}",
            match harness {
                Harness::Claude => "claude",
                Harness::Codex => "codex",
                Harness::CursorAgent => "cursor",
            }
        );
    };
    let root = match harness {
        Harness::Claude => home.join(".claude/projects"),
        Harness::Codex => home.join(".codex/sessions"),
        Harness::CursorAgent => home.join(".cursor/acp-sessions"),
    };
    let record = if harness == Harness::CursorAgent {
        let path = root.join(session_id);
        path.exists().then_some(path)
    } else {
        find_session_record(&root, session_id)
    };
    record.map_or_else(
        || {
            format!(
                "{}:{}:{}",
                match harness {
                    Harness::Claude => "claude",
                    Harness::Codex => "codex",
                    Harness::CursorAgent => "cursor",
                },
                cwd.display(),
                session_id
            )
        },
        |path| path.display().to_string(),
    )
}

pub(super) fn find_session_record(root: &Path, session_id: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_session_record(&path, session_id) {
                return Some(found);
            }
        } else if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains(session_id))
        {
            return Some(path);
        }
    }
    None
}
