use super::*;

#[cfg(unix)]
#[test]
fn onboarding_is_scoped_and_idempotent_in_throwaway_roots() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let config = root.path().join("sub.toml");
    let state = root.path().join("state");
    let binary = env!("CARGO_BIN_EXE_sub");
    std::fs::write(
        &config,
        format!(
            "state_dir = '{}'\n[harnesses.claude]\nbinary = '{binary}'\npermission_mode = 'bypassPermissions'\n[harnesses.codex]\nbinary = '{binary}'\npermission_mode = 'agent'\n[harnesses.cursor]\nbinary = '{binary}'\npermission_mode = 'agent'\n",
            state.display(),
        ),
    )
    .unwrap_or_else(|error| panic!("config: {error}"));
    let claude_config = root.path().join("claude/config.json");
    let claude_skills = root.path().join("claude/skills");
    let codex_config = root.path().join("codex/config.toml");
    let codex_skills = root.path().join("codex/skills");
    let cursor_config = root.path().join("cursor/mcp.json");
    let cursor_skills = root.path().join("cursor/skills");
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
            .env("SUB_CURSOR_CONFIG", &cursor_config)
            .env("SUB_CURSOR_SKILLS_DIR", &cursor_skills)
            .env("SUB_MCP_BINARY", binary)
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
    assert!(!cursor_config.exists());
    assert!(!cursor_skills.exists());

    let first_codex = run(&["codex"]);
    assert!(first_codex.status.success());
    let first_cursor = run(&["cursor"]);
    assert!(first_cursor.status.success());
    assert_cursor_report(&first_cursor.stdout);
    let second = run(&["claude", "codex", "cursor"]);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_idempotent_report(&second.stdout);
    assert!(codex_skills.join("sub-delegation/SKILL.md").is_file());
    assert!(cursor_skills.join("sub-delegation/SKILL.md").is_file());
    let cursor: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&cursor_config).unwrap_or_else(|error| panic!("cursor config: {error}")),
    )
    .unwrap_or_else(|error| panic!("cursor json: {error}"));
    assert_eq!(cursor["mcpServers"]["sub"]["command"], binary);
    assert!(
        std::fs::read_to_string(codex_config)
            .unwrap_or_else(|error| panic!("codex config: {error}"))
            .contains("[mcp_servers.sub]")
    );
}

fn assert_cursor_report(stdout: &[u8]) {
    let report: serde_json::Value =
        serde_json::from_slice(stdout).unwrap_or_else(|error| panic!("cursor report: {error}"));
    assert_eq!(report[0]["bridge"]["status"], "not_required");
}

fn assert_idempotent_report(stdout: &[u8]) {
    let report: serde_json::Value =
        serde_json::from_slice(stdout).unwrap_or_else(|error| panic!("report: {error}"));
    for harness in report.as_array().unwrap_or_else(|| panic!("array")) {
        let bridge = if harness["harness"] == "cursor" {
            "not_required"
        } else {
            "unchanged"
        };
        assert_eq!(harness["bridge"]["status"], bridge);
        assert_eq!(harness["skill"]["status"], "unchanged");
        assert_eq!(harness["mcp"]["status"], "unchanged");
    }
}
