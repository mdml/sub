//! `sub` command-line surface.

mod onboarding;

use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sub_sdk::config::SubConfig;
use sub_sdk::delegation::{AdapterLaunch, Delegator, Harness, LaunchParams, TaskHandle};

fn flag(args: &[String], name: &str) -> Result<String, String> {
    let index = args
        .iter()
        .position(|arg| arg == name)
        .ok_or_else(|| format!("{name} is required"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn optional_flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn state_dir(args: &[String], config: &SubConfig) -> Result<PathBuf, String> {
    if let Some(value) = optional_flag(args, "--state-dir") {
        return Ok(PathBuf::from(value));
    }
    if let Some(value) = &config.state_dir {
        return Ok(value.clone());
    }
    env::var_os("SUB_STATE_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".sub")))
        .ok_or_else(|| "HOME is unset; pass --state-dir".to_owned())
}

fn adapter(harness: Harness, root: &Path, binary: &Path) -> Result<AdapterLaunch, String> {
    match harness {
        Harness::Claude => Ok(AdapterLaunch {
            bridge: sub_adapter_claude::launch(root, binary).map_err(|error| error.to_string())?,
            session_meta: sub_adapter_claude::session_meta(),
            delegation_guard: sub_adapter_claude::DELEGATION_GUARD.to_owned(),
            resume_mechanism: sub_adapter_claude::RESUME_MECHANISM,
        }),
        Harness::Codex => Ok(AdapterLaunch {
            bridge: sub_adapter_codex::launch(root, binary).map_err(|error| error.to_string())?,
            session_meta: sub_adapter_codex::session_meta(),
            delegation_guard: sub_adapter_codex::DELEGATION_GUARD.to_owned(),
            resume_mechanism: sub_adapter_codex::RESUME_MECHANISM,
        }),
    }
}

fn parse_harness(value: &str) -> Result<Harness, String> {
    match value {
        "claude" => Ok(Harness::Claude),
        "codex" => Ok(Harness::Codex),
        _ => Err(format!("unsupported harness: {value}")),
    }
}

fn config() -> Result<sub_sdk::config::LoadedConfig, String> {
    match sub_sdk::config::load() {
        Ok(config) => Ok(config),
        Err(sub_sdk::config::ConfigError::NoConfigHome) => Ok(sub_sdk::config::LoadedConfig {
            config: SubConfig::default(),
            path: PathBuf::new(),
            exists: false,
        }),
        Err(error) => Err(error.to_string()),
    }
}

fn launch_params(args: &[String], config: &SubConfig) -> Result<LaunchParams, String> {
    let harness = parse_harness(&flag(args, "--harness")?)?;
    let defaults = config.harness(harness);
    let harness_binary = optional_flag(args, "--binary")
        .map(PathBuf::from)
        .or_else(|| defaults.map(|entry| entry.binary.clone()))
        .ok_or_else(|| {
            format!(
                "--binary is required when {} is not configured in sub.toml",
                harness_name(harness)
            )
        })?;
    let permission_mode = optional_flag(args, "--permission-mode")
        .or_else(|| defaults.and_then(|entry| entry.permission_mode.clone()))
        .ok_or_else(|| {
            format!(
                "--permission-mode is required when {} has no default in sub.toml",
                harness_name(harness)
            )
        })?;
    Ok(LaunchParams {
        harness,
        prompt: flag(args, "--prompt")?,
        cwd: PathBuf::from(flag(args, "--cwd")?),
        harness_binary,
        model: optional_flag(args, "--model")
            .or_else(|| defaults.and_then(|entry| entry.model.clone())),
        permission_mode,
    })
}

const fn harness_name(harness: Harness) -> &'static str {
    match harness {
        Harness::Claude => "claude",
        Harness::Codex => "codex",
    }
}

fn onboarding_harnesses(args: &[String]) -> Result<Vec<Harness>, String> {
    let mut harnesses = Vec::new();
    for value in args.iter().skip(1).take_while(|arg| !arg.starts_with("--")) {
        let harness = parse_harness(value)?;
        if !harnesses.contains(&harness) {
            harnesses.push(harness);
        }
    }
    if harnesses.is_empty() {
        return Err("usage: sub onboard <claude|codex>... [--state-dir PATH]".to_owned());
    }
    Ok(harnesses)
}

fn mcp_binary() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("SUB_MCP_BINARY") {
        return Ok(PathBuf::from(path));
    }
    Ok(env::current_exe()
        .map_err(|error| error.to_string())?
        .with_file_name("sub-mcp"))
}

fn onboard_command(args: &[String]) -> Result<(), String> {
    let loaded = sub_sdk::config::load().map_err(|error| error.to_string())?;
    if !loaded.exists {
        return Err(format!("sub.toml not found at {}", loaded.path.display()));
    }
    let harnesses = onboarding_harnesses(args)?;
    let root = state_dir(args, &loaded.config)?;
    let reports = onboarding::onboard(
        &harnesses,
        &loaded.config,
        &root,
        &mcp_binary()?,
        &onboarding::Locations::from_environment()?,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&reports).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn recover(args: &[String]) -> Result<(), String> {
    let id = args
        .get(1)
        .ok_or_else(|| "usage: sub recover HANDLE [--state-dir PATH]".to_owned())?;
    let executable = env::current_exe().map_err(|error| error.to_string())?;
    let loaded = config()?;
    let outcome = Delegator::new(state_dir(args, &loaded.config)?, executable)
        .recover(&TaskHandle { id: id.clone() })
        .map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&outcome).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn cancel(args: &[String]) -> Result<(), String> {
    let id = args
        .get(1)
        .ok_or_else(|| "usage: sub cancel HANDLE [--state-dir PATH]".to_owned())?;
    let executable = env::current_exe().map_err(|error| error.to_string())?;
    let loaded = config()?;
    let outcome = Delegator::new(state_dir(args, &loaded.config)?, executable)
        .cancel(&TaskHandle { id: id.clone() })
        .map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&outcome).map_err(|error| error.to_string())?
    );
    Ok(())
}

async fn supervise(args: &[String]) -> Result<(), String> {
    let id = args
        .get(1)
        .ok_or_else(|| "supervisor handle missing".to_owned())?;
    let number = args
        .get(2)
        .ok_or_else(|| "supervisor attempt missing".to_owned())?
        .parse::<u32>()
        .map_err(|error| format!("invalid supervisor attempt: {error}"))?;
    let loaded = config()?;
    sub_sdk::delegation::run_supervisor(
        &state_dir(args, &loaded.config)?,
        &TaskHandle { id: id.clone() },
        number,
    )
    .await
    .map_err(|error| error.to_string())
}

async fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bridge") if args.get(1).map(String::as_str) == Some("install") => {
            let harness = args.get(2).ok_or_else(|| {
                "usage: sub bridge install <claude|codex> [--state-dir PATH]".to_owned()
            })?;
            let loaded = config()?;
            let root = state_dir(&args, &loaded.config)?;
            let binary = match harness.as_str() {
                "claude" => sub_adapter_claude::install_bridge(&root),
                "codex" => sub_adapter_codex::install_bridge(&root),
                _ => return Err(format!("unsupported harness: {harness}")),
            }
            .map_err(|error| error.to_string())?;
            println!("{}", binary.display());
        }
        Some("launch") => {
            let loaded = config()?;
            let root = state_dir(&args, &loaded.config)?;
            let params = launch_params(&args, &loaded.config)?;
            let prepared = adapter(params.harness, &root, &params.harness_binary)?;
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let handle = Delegator::new(root, executable)
                .launch(params, prepared)
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(&handle).map_err(|error| error.to_string())?
            );
        }
        Some("wait") => {
            let id = args.get(1).ok_or_else(|| {
                "usage: sub wait HANDLE [--timeout-seconds N] [--state-dir PATH]".to_owned()
            })?;
            let seconds = optional_flag(&args, "--timeout-seconds")
                .unwrap_or_else(|| "30".to_owned())
                .parse::<u64>()
                .map_err(|error| format!("invalid timeout: {error}"))?;
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let loaded = config()?;
            let outcome = Delegator::new(state_dir(&args, &loaded.config)?, executable)
                .wait(&TaskHandle { id: id.clone() }, Duration::from_secs(seconds))
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome).map_err(|error| error.to_string())?
            );
        }
        Some("recover") => recover(&args)?,
        Some("cancel") => cancel(&args)?,
        Some("list") => {
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let loaded = config()?;
            let tasks = Delegator::new(state_dir(&args, &loaded.config)?, executable)
                .list()
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&tasks).map_err(|error| error.to_string())?
            );
        }
        Some("inspect") => {
            let id = args
                .get(1)
                .ok_or_else(|| "usage: sub inspect HANDLE [--state-dir PATH]".to_owned())?;
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let loaded = config()?;
            let task = Delegator::new(state_dir(&args, &loaded.config)?, executable)
                .inspect(&TaskHandle { id: id.clone() })
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&task).map_err(|error| error.to_string())?
            );
        }
        Some("onboard") => onboard_command(&args)?,
        Some("__supervise") => supervise(&args).await?,
        Some("--version" | "-V") | None => println!("sub {}", sub_sdk::version()),
        _ => {
            return Err(
                "usage: sub <onboard|bridge install|launch|wait|recover|cancel|list|inspect>"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("sub: {error}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_state_dir_wins() {
        let args = vec!["--state-dir".to_owned(), "/tmp/sub-state".to_owned()];
        let config = SubConfig {
            state_dir: Some(PathBuf::from("/tmp/config-state")),
            ..SubConfig::default()
        };
        assert_eq!(
            state_dir(&args, &config),
            Ok(PathBuf::from("/tmp/sub-state"))
        );
        assert_eq!(
            state_dir(&[], &config),
            Ok(PathBuf::from("/tmp/config-state"))
        );
    }
    #[test]
    fn parses_supported_harnesses() {
        assert_eq!(parse_harness("claude"), Ok(Harness::Claude));
        assert_eq!(parse_harness("codex"), Ok(Harness::Codex));
        assert!(parse_harness("cursor").is_err());
    }

    #[test]
    fn launch_resolution_uses_defaults_and_explicit_precedence() {
        let config: SubConfig = toml::from_str(
            "[harnesses.codex]\nbinary = '/configured/codex'\nmodel = 'configured'\npermission_mode = 'agent'\n",
        )
        .unwrap_or_else(|error| panic!("config: {error}"));
        let base = [
            "launch",
            "--harness",
            "codex",
            "--prompt",
            "probe",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned);
        let resolved = launch_params(&base, &config)
            .unwrap_or_else(|error| panic!("configured params: {error}"));
        assert_eq!(resolved.harness_binary, PathBuf::from("/configured/codex"));
        assert_eq!(resolved.model.as_deref(), Some("configured"));
        assert_eq!(resolved.permission_mode, "agent");

        let mut explicit = base.to_vec();
        explicit.extend(
            [
                "--binary",
                "/explicit/codex",
                "--model",
                "explicit",
                "--permission-mode",
                "read-only",
            ]
            .map(str::to_owned),
        );
        let resolved = launch_params(&explicit, &config)
            .unwrap_or_else(|error| panic!("explicit params: {error}"));
        assert_eq!(resolved.harness_binary, PathBuf::from("/explicit/codex"));
        assert_eq!(resolved.model.as_deref(), Some("explicit"));
        assert_eq!(resolved.permission_mode, "read-only");

        let error = launch_params(&base, &SubConfig::default())
            .err()
            .unwrap_or_else(|| panic!("missing binary"));
        assert!(error.contains("--binary is required"));
        let binary_only: SubConfig =
            toml::from_str("[harnesses.codex]\nbinary = '/configured/codex'\n")
                .unwrap_or_else(|error| panic!("config: {error}"));
        assert!(
            launch_params(&base, &binary_only)
                .err()
                .is_some_and(|error| error.contains("--permission-mode is required"))
        );
        assert_eq!(harness_name(Harness::Claude), "claude");
        assert_eq!(
            mcp_binary()
                .unwrap_or_else(|error| panic!("mcp binary: {error}"))
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("sub-mcp")
        );
    }

    #[test]
    fn onboarding_names_are_deduplicated_and_required() {
        let args = [
            "onboard",
            "claude",
            "claude",
            "codex",
            "--state-dir",
            "/tmp",
        ]
        .map(str::to_owned);
        assert_eq!(
            onboarding_harnesses(&args),
            Ok(vec![Harness::Claude, Harness::Codex])
        );
        assert!(onboarding_harnesses(&["onboard".to_owned()]).is_err());
    }
}
