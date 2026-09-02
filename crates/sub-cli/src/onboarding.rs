use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value, json};
use sub_sdk::bridge;
use sub_sdk::config::SubConfig;
use sub_sdk::delegation::Harness;
use toml_edit::{DocumentMut, Item, Table, value};

pub const DELEGATION_SKILL: &str = r"---
name: sub-delegation
description: Delegate bounded work to another supported harness through sub, then retrieve a compact result and durable evidence.
---

# Delegate with `sub`

Use `sub` when a bounded task benefits from parallel work, another harness, or isolation from the manager's context. The manager owns decomposition, evaluation, and integration.

1. Call `sub_launch` with one bounded prompt, the child harness, and its working directory. Keep the returned handle. Configuration supplies binary, model, and permission defaults unless the call overrides them.
2. Call `sub_wait` with the handle. If it is still running, wait again. Prefer the bounded result and artifact references over reconstructing the child transcript.
3. Use `sub_inspect` for task state and normalized evidence, or `sub_list` to find handles. If work is wrong or no longer needed, call `sub_cancel`.
4. If inspection or wait reports `orphaned`, call `sub_recover` once, then wait on the same handle.

Give each child one bounded task with an explicit expected result. Never ask a child to create subagents or delegate again.
";

#[derive(Debug, Clone)]
pub struct Locations {
    claude_config: PathBuf,
    claude_skills: PathBuf,
    codex_config: PathBuf,
    codex_skills: PathBuf,
    cursor_config: PathBuf,
    cursor_skills: PathBuf,
}

impl Locations {
    pub fn from_environment() -> Result<Self, String> {
        let home = env::var_os("HOME").map(PathBuf::from);
        let claude_config = home_path("SUB_CLAUDE_CONFIG", home.as_ref(), ".claude.json")?;
        let claude_skills = home_path("SUB_CLAUDE_SKILLS_DIR", home.as_ref(), ".claude/skills")?;
        let codex_home = env_path("CODEX_HOME").or_else(|| home.as_ref().map(|p| p.join(".codex")));
        let codex_config = home_path("SUB_CODEX_CONFIG", codex_home.as_ref(), "config.toml")?;
        let codex_skills = home_path("SUB_CODEX_SKILLS_DIR", codex_home.as_ref(), "skills")?;
        let cursor_config = home_path("SUB_CURSOR_CONFIG", home.as_ref(), ".cursor/mcp.json")?;
        let cursor_skills = home_path("SUB_CURSOR_SKILLS_DIR", home.as_ref(), ".cursor/skills")?;
        Ok(Self {
            claude_config,
            claude_skills,
            codex_config,
            codex_skills,
            cursor_config,
            cursor_skills,
        })
    }
}

fn home_path(name: &str, home: Option<&PathBuf>, suffix: &str) -> Result<PathBuf, String> {
    env_path(name)
        .or_else(|| home.map(|path| path.join(suffix)))
        .ok_or_else(|| format!("HOME is unset; set {name}"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Created,
    Updated,
    Installed,
    Unchanged,
    NotRequired,
}

#[derive(Debug, Serialize)]
struct Action {
    status: Status,
    path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct Report {
    harness: &'static str,
    bridge: Action,
    skill: Action,
    mcp: Action,
}

#[derive(Clone, Copy)]
pub struct OnboardContext<'a> {
    pub config: &'a SubConfig,
    pub state_dir: &'a Path,
    pub mcp_binary: &'a Path,
    pub locations: &'a Locations,
}

pub fn onboard(harnesses: &[Harness], context: OnboardContext<'_>) -> Result<Vec<Report>, String> {
    if !context.mcp_binary.is_file() {
        return Err(format!(
            "sub-mcp binary not found beside sub: {}",
            context.mcp_binary.display()
        ));
    }
    for &harness in harnesses {
        if context.config.harness(harness).is_none() {
            return Err(format!(
                "{} is not configured in sub.toml",
                harness_name(harness)
            ));
        }
    }
    let mut reports = Vec::with_capacity(harnesses.len());
    for &harness in harnesses {
        reports.push(onboard_harness(harness, &context)?);
    }
    Ok(reports)
}

fn onboard_harness(harness: Harness, context: &OnboardContext<'_>) -> Result<Report, String> {
    let (bridge_status, bridge_path) = match harness {
        Harness::Claude => ensure_bridge(
            context.state_dir,
            sub_adapter_claude::BRIDGE,
            sub_adapter_claude::install_bridge,
        )?,
        Harness::Codex => ensure_bridge(
            context.state_dir,
            sub_adapter_codex::BRIDGE,
            sub_adapter_codex::install_bridge,
        )?,
        Harness::CursorAgent => {
            let binary = &context
                .config
                .harness(Harness::CursorAgent)
                .ok_or_else(|| "cursor is not configured in sub.toml".to_owned())?
                .binary;
            let bridge = sub_adapter_cursor::install_bridge(binary);
            (Status::NotRequired, bridge.binary)
        }
    };
    let (skill_path, config_path) = match harness {
        Harness::Claude => (
            context
                .locations
                .claude_skills
                .join("sub-delegation/SKILL.md"),
            &context.locations.claude_config,
        ),
        Harness::Codex => (
            context
                .locations
                .codex_skills
                .join("sub-delegation/SKILL.md"),
            &context.locations.codex_config,
        ),
        Harness::CursorAgent => (
            context
                .locations
                .cursor_skills
                .join("sub-delegation/SKILL.md"),
            &context.locations.cursor_config,
        ),
    };
    let skill_status = write_if_changed(&skill_path, DELEGATION_SKILL.as_bytes())?;
    let mcp_status = match harness {
        Harness::Claude => register_claude(config_path, context.mcp_binary)?,
        Harness::Codex => register_codex(config_path, context.mcp_binary)?,
        Harness::CursorAgent => register_cursor(config_path, context.mcp_binary)?,
    };
    Ok(Report {
        harness: harness_name(harness),
        bridge: Action {
            status: bridge_status,
            path: bridge_path,
        },
        skill: Action {
            status: skill_status,
            path: skill_path,
        },
        mcp: Action {
            status: mcp_status,
            path: config_path.clone(),
        },
    })
}

fn ensure_bridge(
    state_dir: &Path,
    spec: bridge::BridgeSpec,
    install: fn(&Path) -> Result<PathBuf, bridge::BridgeError>,
) -> Result<(Status, PathBuf), String> {
    match bridge::verify(state_dir, spec) {
        Ok(path) => Ok((Status::Unchanged, path)),
        Err(_) => install(state_dir)
            .map(|path| (Status::Installed, path))
            .map_err(|error| error.to_string()),
    }
}

fn register_claude(path: &Path, mcp_binary: &Path) -> Result<Status, String> {
    register_json(path, json!({"type":"stdio","command":mcp_binary,"args":[]}))
}

fn register_cursor(path: &Path, mcp_binary: &Path) -> Result<Status, String> {
    register_json(path, json!({"command":mcp_binary,"args":[]}))
}

fn register_json(path: &Path, server: Value) -> Result<Status, String> {
    let mut root = read_json_object(path)?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| format!("{}: mcpServers must be an object", path.display()))?;
    servers.insert("sub".to_owned(), server);
    let bytes =
        serde_json::to_vec_pretty(&Value::Object(root)).map_err(|error| error.to_string())?;
    write_if_changed(path, &bytes)
}

fn read_json_object(path: &Path) -> Result<Map<String, Value>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes)
            .map_err(|error| format!("cannot parse {}: {error}", path.display()))?
            .as_object()
            .cloned()
            .ok_or_else(|| format!("{} must contain a JSON object", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Map::new()),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

fn register_codex(path: &Path, mcp_binary: &Path) -> Result<Status, String> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let mut document = contents
        .parse::<DocumentMut>()
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    if !document.as_table().contains_key("mcp_servers") {
        document["mcp_servers"] = Item::Table(Table::new());
    }
    let servers = document["mcp_servers"]
        .as_table_mut()
        .ok_or_else(|| format!("{}: mcp_servers must be a table", path.display()))?;
    if !servers.contains_key("sub") {
        servers.insert("sub", Item::Table(Table::new()));
    }
    if !servers["sub"].is_table() {
        return Err(format!(
            "{}: mcp_servers.sub must be a table",
            path.display()
        ));
    }
    document["mcp_servers"]["sub"]["command"] = value(mcp_binary.to_string_lossy().into_owned());
    let rendered = document.to_string();
    write_if_changed(path, rendered.as_bytes())
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<Status, String> {
    match fs::read(path) {
        Ok(existing) if existing == bytes => return Ok(Status::Unchanged),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            write_new(path, bytes)?;
            return Ok(Status::Created);
        }
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    }
    write_new(path, bytes)?;
    Ok(Status::Updated)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
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

    fn onboard_test(
        harnesses: &[Harness],
        context: OnboardContext<'_>,
    ) -> Result<Vec<Report>, String> {
        onboard(harnesses, context)
    }

    #[test]
    fn registration_preserves_unrelated_configuration_and_is_idempotent() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let claude = root.path().join("claude.json");
        fs::write(&claude, r#"{"theme":"dark"}"#).unwrap_or_else(|error| panic!("write: {error}"));
        assert_eq!(
            register_claude(&claude, Path::new("/bin/sub-mcp"))
                .unwrap_or_else(|error| panic!("register: {error}")),
            Status::Updated
        );
        assert_eq!(
            register_claude(&claude, Path::new("/bin/sub-mcp"))
                .unwrap_or_else(|error| panic!("register: {error}")),
            Status::Unchanged
        );
        let value: Value = serde_json::from_slice(
            &fs::read(&claude).unwrap_or_else(|error| panic!("read: {error}")),
        )
        .unwrap_or_else(|error| panic!("json: {error}"));
        assert_eq!(value["theme"], "dark");

        let codex = root.path().join("config.toml");
        fs::write(&codex, "model = 'existing'\n").unwrap_or_else(|error| panic!("write: {error}"));
        assert_eq!(
            register_codex(&codex, Path::new("/bin/sub-mcp"))
                .unwrap_or_else(|error| panic!("register: {error}")),
            Status::Updated
        );
        assert_eq!(
            register_codex(&codex, Path::new("/bin/sub-mcp"))
                .unwrap_or_else(|error| panic!("register: {error}")),
            Status::Unchanged
        );
        let contents = fs::read_to_string(codex).unwrap_or_else(|error| panic!("read: {error}"));
        assert!(contents.contains("model = 'existing'"));
        assert!(contents.contains("/bin/sub-mcp"));
    }

    #[test]
    fn skill_writer_repairs_changed_content() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let path = root.path().join("skills/sub-delegation/SKILL.md");
        assert_eq!(
            write_if_changed(&path, DELEGATION_SKILL.as_bytes())
                .unwrap_or_else(|error| panic!("create: {error}")),
            Status::Created
        );
        assert_eq!(
            write_if_changed(&path, DELEGATION_SKILL.as_bytes())
                .unwrap_or_else(|error| panic!("same: {error}")),
            Status::Unchanged
        );
        fs::write(&path, "stale").unwrap_or_else(|error| panic!("stale: {error}"));
        assert_eq!(
            write_if_changed(&path, DELEGATION_SKILL.as_bytes())
                .unwrap_or_else(|error| panic!("repair: {error}")),
            Status::Updated
        );
    }

    #[test]
    fn onboarding_validates_before_writing() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let locations = Locations {
            claude_config: root.path().join("claude.json"),
            claude_skills: root.path().join("claude-skills"),
            codex_config: root.path().join("codex.toml"),
            codex_skills: root.path().join("codex-skills"),
            cursor_config: root.path().join("cursor.json"),
            cursor_skills: root.path().join("cursor-skills"),
        };
        let missing_binary = onboard_test(
            &[Harness::Claude],
            OnboardContext {
                config: &SubConfig::default(),
                state_dir: root.path(),
                mcp_binary: &root.path().join("missing-sub-mcp"),
                locations: &locations,
            },
        )
        .err()
        .unwrap_or_else(|| panic!("missing binary error"));
        assert!(missing_binary.contains("sub-mcp binary not found"));

        let unconfigured = onboard_test(
            &[Harness::Claude],
            OnboardContext {
                config: &SubConfig::default(),
                state_dir: root.path(),
                mcp_binary: Path::new("/bin/true"),
                locations: &locations,
            },
        )
        .err()
        .unwrap_or_else(|| panic!("unconfigured error"));
        assert!(unconfigured.contains("not configured"));
        assert!(!locations.claude_config.exists());

        let cursor_config: SubConfig = toml::from_str(
            "[harnesses.cursor]\nbinary = '/bin/cursor-agent'\npermission_mode = 'agent'\n",
        )
        .unwrap_or_else(|error| panic!("cursor config: {error}"));
        let reports = onboard_test(
            &[Harness::CursorAgent],
            OnboardContext {
                config: &cursor_config,
                state_dir: root.path(),
                mcp_binary: Path::new("/bin/true"),
                locations: &locations,
            },
        )
        .unwrap_or_else(|error| panic!("cursor onboard: {error}"));
        assert_eq!(reports[0].bridge.status, Status::NotRequired);
        assert!(locations.cursor_config.is_file());
        assert!(
            locations
                .cursor_skills
                .join("sub-delegation/SKILL.md")
                .is_file()
        );
    }

    #[test]
    fn registration_rejects_invalid_existing_shapes() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let claude = root.path().join("claude.json");
        fs::write(&claude, "[]").unwrap_or_else(|error| panic!("write: {error}"));
        assert!(register_claude(&claude, Path::new("/bin/sub-mcp")).is_err());
        fs::write(&claude, r#"{"mcpServers":"wrong"}"#)
            .unwrap_or_else(|error| panic!("write: {error}"));
        assert!(register_claude(&claude, Path::new("/bin/sub-mcp")).is_err());
        let cursor = root.path().join("cursor.json");
        fs::write(&cursor, r#"{"mcpServers":"wrong"}"#)
            .unwrap_or_else(|error| panic!("write: {error}"));
        assert!(register_cursor(&cursor, Path::new("/bin/sub-mcp")).is_err());

        let codex = root.path().join("config.toml");
        fs::write(&codex, "not valid = [").unwrap_or_else(|error| panic!("write: {error}"));
        assert!(register_codex(&codex, Path::new("/bin/sub-mcp")).is_err());
        fs::write(&codex, "mcp_servers = 'wrong'\n")
            .unwrap_or_else(|error| panic!("write: {error}"));
        assert!(register_codex(&codex, Path::new("/bin/sub-mcp")).is_err());
        fs::write(&codex, "[mcp_servers]\nsub = 'wrong'\n")
            .unwrap_or_else(|error| panic!("write: {error}"));
        assert!(register_codex(&codex, Path::new("/bin/sub-mcp")).is_err());

        let directory = root.path().join("not-a-file");
        fs::create_dir(&directory).unwrap_or_else(|error| panic!("directory: {error}"));
        assert!(read_json_object(&directory).is_err());
        assert!(register_codex(&directory, Path::new("/bin/sub-mcp")).is_err());
        assert!(write_if_changed(&directory, b"content").is_err());
    }
}
