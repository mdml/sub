//! MCP stdio surface for delegated-task controls and pinned bridge installation.

use std::env;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use sub_sdk::config::SubConfig;
use sub_sdk::delegation::{AdapterLaunch, Delegator, Harness, LaunchParams, TaskHandle};

fn default_state_dir(value: Option<&str>, config: &SubConfig) -> Result<PathBuf, String> {
    if let Some(value) = value {
        return Ok(PathBuf::from(value));
    }
    if let Some(value) = &config.state_dir {
        return Ok(value.clone());
    }
    env::var_os("SUB_STATE_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".sub")))
        .ok_or_else(|| "HOME is unset; provide state_dir".to_owned())
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

fn string_arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, String> {
    args.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name} is required"))
}

fn parse_harness(value: &str) -> Result<Harness, String> {
    match value {
        "claude" => Ok(Harness::Claude),
        "codex" => Ok(Harness::Codex),
        "cursor" => Ok(Harness::CursorAgent),
        _ => Err(format!("unsupported harness: {value}")),
    }
}

fn launch_params(args: &Value, config: &SubConfig) -> Result<LaunchParams, String> {
    let harness = parse_harness(string_arg(args, "harness")?)?;
    let defaults = config.harness(harness);
    let harness_binary = args
        .get("binary")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| defaults.map(|entry| entry.binary.clone()))
        .ok_or_else(|| {
            "binary is required when the harness is not configured in sub.toml".to_owned()
        })?;
    let permission_mode = args
        .get("permission_mode")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| defaults.and_then(|entry| entry.permission_mode.clone()))
        .ok_or_else(|| {
            "permission_mode is required when the harness has no default in sub.toml".to_owned()
        })?;
    Ok(LaunchParams {
        harness,
        prompt: string_arg(args, "prompt")?.to_owned(),
        cwd: PathBuf::from(string_arg(args, "cwd")?),
        harness_binary,
        model: args
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| defaults.and_then(|entry| entry.model.clone())),
        permission_mode,
    })
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
        Harness::CursorAgent => Ok(AdapterLaunch {
            bridge: sub_adapter_cursor::launch(binary),
            session_meta: sub_adapter_cursor::session_meta(),
            delegation_guard: sub_adapter_cursor::DELEGATION_GUARD.to_owned(),
            resume_mechanism: sub_adapter_cursor::RESUME_MECHANISM,
        }),
    }
}

fn tool_state_dir(args: &Value, config: &SubConfig) -> Result<PathBuf, String> {
    default_state_dir(args.get("state_dir").and_then(Value::as_str), config)
}

fn install_bridge_tool(args: &Value, config: &SubConfig) -> Result<Value, String> {
    let harness = string_arg(args, "harness")?;
    let root = tool_state_dir(args, config)?;
    match harness {
        "claude" => sub_adapter_claude::install_bridge(&root)
            .map(|binary| json!({"bridge_binary": binary}))
            .map_err(|error| error.to_string()),
        "codex" => sub_adapter_codex::install_bridge(&root)
            .map(|binary| json!({"bridge_binary": binary}))
            .map_err(|error| error.to_string()),
        "cursor" => {
            let configured = config
                .harness(Harness::CursorAgent)
                .ok_or_else(|| "cursor is not configured in sub.toml".to_owned())?;
            let bridge = sub_adapter_cursor::install_bridge(&configured.binary);
            Ok(
                json!({"bridge_binary": bridge.binary, "required": false, "message": bridge.message}),
            )
        }
        _ => Err(format!("unsupported harness: {harness}")),
    }
}

fn launch_tool(args: &Value, config: &SubConfig) -> Result<Value, String> {
    let root = tool_state_dir(args, config)?;
    let params = launch_params(args, config)?;
    let prepared = adapter(params.harness, &root, &params.harness_binary)?;
    let executable = env::current_exe().map_err(|error| error.to_string())?;
    let handle = Delegator::new(root, executable)
        .launch(params, prepared)
        .map_err(|error| error.to_string())?;
    serde_json::to_value(handle).map_err(|error| error.to_string())
}

async fn call_tool(name: &str, args: &Value) -> Result<Value, String> {
    let loaded = config()?;
    match name {
        "sub_bridge_install" => install_bridge_tool(args, &loaded.config),
        "sub_launch" => launch_tool(args, &loaded.config),
        "sub_wait" => {
            let root = tool_state_dir(args, &loaded.config)?;
            let seconds = args
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(30);
            let handle = TaskHandle {
                id: string_arg(args, "handle")?.to_owned(),
            };
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let result = Delegator::new(root, executable)
                .wait(&handle, Duration::from_secs(seconds))
                .await
                .map_err(|error| error.to_string())?;
            serde_json::to_value(result).map_err(|error| error.to_string())
        }
        "sub_recover" => {
            let root = tool_state_dir(args, &loaded.config)?;
            let handle = TaskHandle {
                id: string_arg(args, "handle")?.to_owned(),
            };
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let result = Delegator::new(root, executable)
                .recover(&handle)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(result).map_err(|error| error.to_string())
        }
        "sub_cancel" => {
            let root = tool_state_dir(args, &loaded.config)?;
            let handle = TaskHandle {
                id: string_arg(args, "handle")?.to_owned(),
            };
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let result = Delegator::new(root, executable)
                .cancel(&handle)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(result).map_err(|error| error.to_string())
        }
        "sub_list" => {
            let root = tool_state_dir(args, &loaded.config)?;
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let result = Delegator::new(root, executable)
                .list()
                .map_err(|error| error.to_string())?;
            serde_json::to_value(result).map_err(|error| error.to_string())
        }
        "sub_inspect" => {
            let root = tool_state_dir(args, &loaded.config)?;
            let handle = TaskHandle {
                id: string_arg(args, "handle")?.to_owned(),
            };
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let result = Delegator::new(root, executable)
                .inspect(&handle)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(result).map_err(|error| error.to_string())
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}

fn tools() -> Value {
    json!({"tools":[
        {"name":"sub_launch","description":"Launch one bounded delegated task and immediately return its handle. Configured harness defaults supply omitted binary, model, and permission mode.","inputSchema":{"type":"object","required":["harness","prompt","cwd"],"properties":{"harness":{"type":"string","enum":["claude","codex","cursor"]},"prompt":{"type":"string"},"cwd":{"type":"string"},"binary":{"type":"string"},"model":{"type":"string"},"permission_mode":{"type":"string"},"state_dir":{"type":"string"}}}},
        {"name":"sub_wait","description":"Wait up to a timeout for a delegated task result; re-wait with the same handle if still running.","inputSchema":{"type":"object","required":["handle"],"properties":{"handle":{"type":"string"},"timeout_seconds":{"type":"integer","minimum":0},"state_dir":{"type":"string"}}}},
        {"name":"sub_recover","description":"Start a new attempt that resumes an orphaned task's recorded harness session.","inputSchema":{"type":"object","required":["handle"],"properties":{"handle":{"type":"string"},"state_dir":{"type":"string"}}}},
        {"name":"sub_cancel","description":"Request cancellation of one task's latest attempt and return the delivery disposition immediately.","inputSchema":{"type":"object","required":["handle"],"properties":{"handle":{"type":"string"},"state_dir":{"type":"string"}}}},
        {"name":"sub_list","description":"List delegated tasks by reading the state directory without contacting supervisors or harnesses.","inputSchema":{"type":"object","properties":{"state_dir":{"type":"string"}}}},
        {"name":"sub_inspect","description":"Inspect one task's status, normalized events, cost, and tokens by reading the state directory.","inputSchema":{"type":"object","required":["handle"],"properties":{"handle":{"type":"string"},"state_dir":{"type":"string"}}}},
        {"name":"sub_bridge_install","description":"Install or verify a harness's ACP transport. Cursor uses native ACP and reports that no bridge is required.","inputSchema":{"type":"object","required":["harness"],"properties":{"harness":{"type":"string","enum":["claude","codex","cursor"]},"state_dir":{"type":"string"}}}}
    ]})
}

async fn respond(request: Value) -> Option<Value> {
    let id = request.get("id").cloned()?;
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let result = match method {
        "initialize" => Ok(
            json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"sub-mcp","version":sub_sdk::version()}}),
        ),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools()),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            call_tool(name, &args).await.map(|value| json!({"content":[{"type":"text","text":value.to_string()}],"structuredContent":value,"isError":false}))
        }
        _ => Err(format!("method not found: {method}")),
    };
    Some(match result {
        Ok(value) => json!({"jsonrpc":"2.0","id":id,"result":value}),
        Err(error) => {
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":error}],"isError":true}})
        }
    })
}

async fn serve() -> Result<(), String> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line).map_err(|error| error.to_string())?;
        if let Some(response) = respond(request).await {
            serde_json::to_writer(&mut stdout, &response).map_err(|error| error.to_string())?;
            stdout.write_all(b"\n").map_err(|error| error.to_string())?;
            stdout.flush().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = if args.first().map(String::as_str) == Some("__supervise") {
        let id = args
            .get(1)
            .cloned()
            .ok_or_else(|| "supervisor handle missing".to_owned());
        let number = args
            .get(2)
            .ok_or_else(|| "supervisor attempt missing".to_owned())
            .and_then(|value| value.parse::<u32>().map_err(|error| error.to_string()));
        let root = args
            .iter()
            .position(|arg| arg == "--state-dir")
            .and_then(|index| args.get(index + 1))
            .map(String::as_str);
        let loaded = config();
        match id
            .and_then(|id| number.map(|number| (id, number)))
            .and_then(|(id, number)| {
                loaded.and_then(|loaded| {
                    default_state_dir(root, &loaded.config).map(|root| (id, number, root))
                })
            }) {
            Ok((id, number, root)) => {
                sub_sdk::delegation::run_supervisor(&root, &TaskHandle { id }, number)
                    .await
                    .map_err(|error| error.to_string())
            }
            Err(error) => Err(error),
        }
    } else if args
        .first()
        .is_some_and(|arg| arg == "--version" || arg == "-V")
    {
        println!("sub-mcp {}", sub_sdk::version());
        Ok(())
    } else {
        serve().await
    };
    if let Err(error) = result {
        eprintln!("sub-mcp: {error}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lists_all_beta_tools() {
        let listed = tools();
        let names = listed["tools"]
            .as_array()
            .unwrap_or_else(|| panic!("tools array"))
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "sub_launch",
                "sub_wait",
                "sub_recover",
                "sub_cancel",
                "sub_list",
                "sub_inspect",
                "sub_bridge_install"
            ]
        );
        assert_eq!(parse_harness("cursor"), Ok(Harness::CursorAgent));
        let config = SubConfig {
            harnesses: sub_sdk::config::HarnessConfigs {
                cursor: Some(sub_sdk::config::HarnessConfig {
                    binary: PathBuf::from("/bin/cursor-agent"),
                    model: None,
                    permission_mode: Some("agent".to_owned()),
                }),
                ..sub_sdk::config::HarnessConfigs::default()
            },
            ..SubConfig::default()
        };
        let prepared = adapter(
            Harness::CursorAgent,
            Path::new("/unused"),
            Path::new("/bin/cursor-agent"),
        )
        .unwrap_or_else(|error| panic!("cursor adapter: {error}"));
        assert_eq!(prepared.bridge.args(), &["acp"]);
        assert_eq!(
            prepared.resume_mechanism,
            sub_sdk::delegation::ResumeMechanism::Load
        );
        let installed =
            install_bridge_tool(&json!({"harness":"cursor","state_dir":"/unused"}), &config)
                .unwrap_or_else(|error| panic!("cursor bridge: {error}"));
        assert_eq!(installed["required"], false);
        assert_eq!(installed["bridge_binary"], "/bin/cursor-agent");
    }

    #[tokio::test]
    async fn protocol_methods_respond() {
        let initialized = respond(json!({"jsonrpc":"2.0","id":1,"method":"initialize"}))
            .await
            .unwrap_or_else(|| panic!("response"));
        assert_eq!(initialized["result"]["serverInfo"]["name"], "sub-mcp");
        let ping = respond(json!({"jsonrpc":"2.0","id":2,"method":"ping"}))
            .await
            .unwrap_or_else(|| panic!("response"));
        assert_eq!(ping["result"], json!({}));
        let listed = respond(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}))
            .await
            .unwrap_or_else(|| panic!("response"));
        assert_eq!(listed["result"]["tools"].as_array().map(Vec::len), Some(7));
        let launch_harnesses =
            &listed["result"]["tools"][0]["inputSchema"]["properties"]["harness"]["enum"];
        assert_eq!(launch_harnesses, &json!(["claude", "codex", "cursor"]));
        let bridge_harnesses =
            &listed["result"]["tools"][6]["inputSchema"]["properties"]["harness"]["enum"];
        assert_eq!(bridge_harnesses, &json!(["claude", "codex", "cursor"]));
        let missing = respond(json!({"jsonrpc":"2.0","id":4,"method":"unknown"}))
            .await
            .unwrap_or_else(|| panic!("response"));
        assert_eq!(missing["result"]["isError"], true);
        assert!(
            respond(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn tool_errors_cover_public_argument_shapes() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let root_text = root.path().to_string_lossy();
        let wait_error = call_tool(
            "sub_wait",
            &json!({"handle":"tsk_000000000000000000000000","timeout_seconds":0,"state_dir":root_text}),
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("wait error"));
        assert!(wait_error.contains("unknown task"));
        let recover_error = call_tool(
            "sub_recover",
            &json!({"handle":"tsk_000000000000000000000000","state_dir":root_text}),
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("recover error"));
        assert!(recover_error.contains("unknown task"));
        let cancel_error = call_tool(
            "sub_cancel",
            &json!({"handle":"tsk_000000000000000000000000","state_dir":root_text}),
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("cancel error"));
        assert!(cancel_error.contains("unknown task"));
        let install_error = call_tool(
            "sub_bridge_install",
            &json!({"harness":"unknown","state_dir":root_text}),
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("install error"));
        assert!(install_error.contains("unsupported"));
        let unknown_error = call_tool("unknown", &json!({}))
            .await
            .err()
            .unwrap_or_else(|| panic!("unknown error"));
        assert!(unknown_error.contains("unknown tool"));
        let missing_binary = call_tool(
            "sub_launch",
            &json!({"harness":"codex","prompt":"probe","cwd":root_text}),
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("missing binary"));
        assert!(missing_binary.contains("binary is required"));
        let missing_permission = call_tool(
            "sub_launch",
            &json!({"harness":"codex","prompt":"probe","cwd":root_text,"binary":"/bin/true"}),
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("missing permission"));
        assert!(missing_permission.contains("permission_mode is required"));
        for harness in ["claude", "codex"] {
            let args = json!({"harness":harness,"prompt":"probe","cwd":root_text,"binary":std::env::current_exe().unwrap_or_else(|error| panic!("exe: {error}")),"permission_mode":"agent","state_dir":root_text});
            let error = call_tool("sub_launch", &args)
                .await
                .err()
                .unwrap_or_else(|| panic!("launch error"));
            assert!(error.contains("sub bridge install"));
        }
        let response = respond(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"sub_wait","arguments":{"handle":"tsk_000000000000000000000000","timeout_seconds":0,"state_dir":root_text}}})).await.unwrap_or_else(|| panic!("response"));
        assert_eq!(response["result"]["isError"], true);
    }
}
