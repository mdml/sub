use std::env;
use std::fs;
use std::path::Path;

use sub_sdk::delegation::{DelegatedTask, Delegator, Harness, TaskHandle};

use crate::{Arguments, config, state_dir};

const HELP: &str = "usage: sub report HANDLE [--state-dir PATH]

Draft a GitHub issue command from durable task evidence. This command never submits it.

The draft keeps only the first line of the delegated prompt and normalized `sub
inspect` evidence. It replaces home directories and hostnames, and excludes
later prompt text, result summaries, native transcripts, supervisor logs, and
other state known to contain user content. Review the draft before running it.";

pub(crate) fn report_command(args: &Arguments) -> Result<(), String> {
    if args.get(1).map(String::as_str) == Some("--help") {
        println!("{HELP}");
        return Ok(());
    }
    let id = args
        .get(1)
        .ok_or_else(|| "usage: sub report HANDLE [--state-dir PATH]".to_owned())?;
    let handle = TaskHandle { id: id.clone() };
    let loaded = config()?;
    let root = state_dir(args, &loaded.config)?;
    let executable = env::current_exe().map_err(|error| error.to_string())?;
    let inspection = Delegator::new(&root, executable)
        .inspect(&handle)
        .map_err(|error| error.to_string())?;
    let task: DelegatedTask = serde_json::from_slice(
        &fs::read(root.join("tasks").join(id).join("task.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let prompt = task.params.prompt.lines().next().unwrap_or_default();
    let diagnostics =
        serde_json::to_string_pretty(&inspection).map_err(|error| error.to_string())?;
    let body = scrub(
        &format!(
            "## sub version/tag\n\nsub {}\n\n## Manager harness + version\n\n<!-- Fill in the manager harness and version. -->\n\n## Child harness + version\n\n{} / <!-- Fill in the child harness version. -->\n\n## What was delegated\n\n{}\n\n## Expected behavior\n\n<!-- Fill in what should have happened. -->\n\n## Actual behavior\n\n<!-- Fill in what happened instead. -->\n\n## Diagnostics\n\n> Review and scrub this draft before submission. In particular, redact home paths, prompt content, hostnames, and any other user content that remains.\n\nTask handle: `{}`\n\nScrubbed `sub inspect` output:\n\n```json\n{}\n```",
            sub_sdk::version(),
            harness_name(task.params.harness),
            prompt,
            handle.id,
            diagnostics
        ),
        env::var_os("HOME").as_deref().map(Path::new),
        env::var("HOSTNAME").ok().as_deref(),
    );
    let title = format!("sub task {} misbehaved", handle.id);
    println!(
        "gh issue create -R mdml/sub --title {} --body {} --label {}",
        shell_quote(&title),
        shell_quote(&body),
        shell_quote(&format!("harness:{}", harness_name(task.params.harness)))
    );
    Ok(())
}

fn scrub(text: &str, home: Option<&Path>, hostname: Option<&str>) -> String {
    let mut scrubbed = text.to_owned();
    if let Some(home) = home.filter(|path| path.as_os_str().len() > 1) {
        scrubbed = scrubbed.replace(&home.to_string_lossy().to_string(), "[HOME]");
    }
    if let Some(hostname) = hostname.filter(|value| value.len() > 1) {
        scrubbed = scrubbed.replace(hostname, "[HOSTNAME]");
    }
    scrubbed
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

const fn harness_name(harness: Harness) -> &'static str {
    match harness {
        Harness::Claude => "claude",
        Harness::Codex => "codex",
        Harness::CursorAgent => "cursor",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_replaces_known_machine_identifiers() {
        assert_eq!(
            scrub(
                "/home/alice/project on build-host",
                Some(Path::new("/home/alice")),
                Some("build-host")
            ),
            "[HOME]/project on [HOSTNAME]"
        );
        assert_eq!(scrub("/tmp", Some(Path::new("/")), None), "/tmp");
        assert_eq!(scrub("a task", None, Some("a")), "a task");
    }

    #[test]
    fn shell_quote_handles_apostrophes() {
        assert_eq!(shell_quote("child's task"), "'child'\"'\"'s task'");
    }
}
