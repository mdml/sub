//! MCP stdio surface for delegated-task controls and pinned bridge installation.

use std::env;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use sub_sdk::delegation::{AdapterLaunch, Delegator, Harness, LaunchParams, TaskHandle};

fn default_state_dir(value: Option<&str>) -> Result<PathBuf, String> {
    if let Some(value) = value {
        return Ok(PathBuf::from(value));
    }
    env::var_os("SUB_STATE_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".sub")))
        .ok_or_else(|| "HOME is unset; provide state_dir".to_owned())
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
        _ => Err(format!("unsupported harness: {value}")),
    }
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

async fn call_tool(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "sub_bridge_install" => {
            let harness = string_arg(args, "harness")?;
            let root = default_state_dir(args.get("state_dir").and_then(Value::as_str))?;
            let binary = match harness {
                "claude" => sub_adapter_claude::install_bridge(&root),
                "codex" => sub_adapter_codex::install_bridge(&root),
                _ => return Err(format!("unsupported harness: {harness}")),
            }
            .map_err(|error| error.to_string())?;
            Ok(json!({"bridge_binary": binary}))
        }
        "sub_launch" => {
            let root = default_state_dir(args.get("state_dir").and_then(Value::as_str))?;
            let harness = parse_harness(string_arg(args, "harness")?)?;
            let harness_binary = PathBuf::from(string_arg(args, "binary")?);
            let params = LaunchParams {
                harness,
                prompt: string_arg(args, "prompt")?.to_owned(),
                cwd: PathBuf::from(string_arg(args, "cwd")?),
                harness_binary: harness_binary.clone(),
                model: args.get("model").and_then(Value::as_str).map(str::to_owned),
                permission_mode: string_arg(args, "permission_mode")?.to_owned(),
            };
            let prepared = adapter(harness, &root, &harness_binary)?;
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let handle = Delegator::new(root, executable)
                .launch(params, prepared)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(handle).map_err(|error| error.to_string())
        }
        "sub_wait" => {
            let root = default_state_dir(args.get("state_dir").and_then(Value::as_str))?;
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
            let root = default_state_dir(args.get("state_dir").and_then(Value::as_str))?;
            let handle = TaskHandle {
                id: string_arg(args, "handle")?.to_owned(),
            };
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let result = Delegator::new(root, executable)
                .recover(&handle)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(result).map_err(|error| error.to_string())
        }
        "sub_list" => {
            let root = default_state_dir(args.get("state_dir").and_then(Value::as_str))?;
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let result = Delegator::new(root, executable)
                .list()
                .map_err(|error| error.to_string())?;
            serde_json::to_value(result).map_err(|error| error.to_string())
        }
        "sub_inspect" => {
            let root = default_state_dir(args.get("state_dir").and_then(Value::as_str))?;
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
        {"name":"sub_launch","description":"Launch one bounded delegated task and immediately return its handle.","inputSchema":{"type":"object","required":["harness","prompt","cwd","binary","permission_mode"],"properties":{"harness":{"type":"string","enum":["claude","codex"]},"prompt":{"type":"string"},"cwd":{"type":"string"},"binary":{"type":"string"},"model":{"type":"string"},"permission_mode":{"type":"string"},"state_dir":{"type":"string"}}}},
        {"name":"sub_wait","description":"Wait up to a timeout for a delegated task result; re-wait with the same handle if still running.","inputSchema":{"type":"object","required":["handle"],"properties":{"handle":{"type":"string"},"timeout_seconds":{"type":"integer","minimum":0},"state_dir":{"type":"string"}}}},
        {"name":"sub_recover","description":"Start a new attempt that resumes an orphaned task's recorded harness session.","inputSchema":{"type":"object","required":["handle"],"properties":{"handle":{"type":"string"},"state_dir":{"type":"string"}}}},
        {"name":"sub_list","description":"List delegated tasks by reading the state directory without contacting supervisors or harnesses.","inputSchema":{"type":"object","properties":{"state_dir":{"type":"string"}}}},
        {"name":"sub_inspect","description":"Inspect one task's status, normalized events, cost, and tokens by reading the state directory.","inputSchema":{"type":"object","required":["handle"],"properties":{"handle":{"type":"string"},"state_dir":{"type":"string"}}}},
        {"name":"sub_bridge_install","description":"Explicitly install one exact pinned ACP bridge and write its integrity manifest.","inputSchema":{"type":"object","required":["harness"],"properties":{"harness":{"type":"string","enum":["claude","codex"]},"state_dir":{"type":"string"}}}}
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
        match id
            .and_then(|id| number.map(|number| (id, number)))
            .and_then(|(id, number)| default_state_dir(root).map(|root| (id, number, root)))
        {
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
                "sub_list",
                "sub_inspect",
                "sub_bridge_install"
            ]
        );
        assert!(parse_harness("cursor").is_err());
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
        assert_eq!(listed["result"]["tools"].as_array().map(Vec::len), Some(6));
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
        let install_error = call_tool(
            "sub_bridge_install",
            &json!({"harness":"cursor","state_dir":root_text}),
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
