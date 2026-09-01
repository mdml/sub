//! `sub` command-line surface.

use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

fn state_dir(args: &[String]) -> Result<PathBuf, String> {
    if let Some(value) = optional_flag(args, "--state-dir") {
        return Ok(PathBuf::from(value));
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
        }),
        Harness::Codex => Ok(AdapterLaunch {
            bridge: sub_adapter_codex::launch(root, binary).map_err(|error| error.to_string())?,
            session_meta: sub_adapter_codex::session_meta(),
            delegation_guard: sub_adapter_codex::DELEGATION_GUARD.to_owned(),
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

async fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bridge") if args.get(1).map(String::as_str) == Some("install") => {
            let harness = args.get(2).ok_or_else(|| {
                "usage: sub bridge install <claude|codex> [--state-dir PATH]".to_owned()
            })?;
            let root = state_dir(&args)?;
            let binary = match harness.as_str() {
                "claude" => sub_adapter_claude::install_bridge(&root),
                "codex" => sub_adapter_codex::install_bridge(&root),
                _ => return Err(format!("unsupported harness: {harness}")),
            }
            .map_err(|error| error.to_string())?;
            println!("{}", binary.display());
        }
        Some("launch") => {
            let root = state_dir(&args)?;
            let harness = parse_harness(&flag(&args, "--harness")?)?;
            let harness_binary = PathBuf::from(flag(&args, "--binary")?);
            let params = LaunchParams {
                harness,
                prompt: flag(&args, "--prompt")?,
                cwd: PathBuf::from(flag(&args, "--cwd")?),
                harness_binary: harness_binary.clone(),
                model: optional_flag(&args, "--model"),
                permission_mode: flag(&args, "--permission-mode")?,
            };
            let prepared = adapter(harness, &root, &harness_binary)?;
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
            let outcome = Delegator::new(state_dir(&args)?, executable)
                .wait(&TaskHandle { id: id.clone() }, Duration::from_secs(seconds))
                .await
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome).map_err(|error| error.to_string())?
            );
        }
        Some("list") => {
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let tasks = Delegator::new(state_dir(&args)?, executable)
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
            let task = Delegator::new(state_dir(&args)?, executable)
                .inspect(&TaskHandle { id: id.clone() })
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&task).map_err(|error| error.to_string())?
            );
        }
        Some("__supervise") => {
            let id = args
                .get(1)
                .ok_or_else(|| "supervisor handle missing".to_owned())?;
            sub_sdk::delegation::run_supervisor(&state_dir(&args)?, &TaskHandle { id: id.clone() })
                .await
                .map_err(|error| error.to_string())?;
        }
        Some("--version" | "-V") | None => println!("sub {}", sub_sdk::version()),
        _ => return Err("usage: sub <bridge install|launch|wait|list|inspect>".to_owned()),
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
        assert_eq!(state_dir(&args), Ok(PathBuf::from("/tmp/sub-state")));
    }
    #[test]
    fn parses_supported_harnesses() {
        assert_eq!(parse_harness("claude"), Ok(Harness::Claude));
        assert_eq!(parse_harness("codex"), Ok(Harness::Codex));
        assert!(parse_harness("cursor").is_err());
    }
}
