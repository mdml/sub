//! Disposable prototype for the ACP boundary spike.
//!
//! Launches an ACP agent process, opens one session in `--cwd`, sends one prompt,
//! auto-approves every permission request, records every `session/update`
//! notification to a JSONL file, and writes a `result.json` with the handle
//! (session id), stop reason, last assistant text and usage figures.

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionNotification, SessionUpdate, TextContent,
};
use agent_client_protocol::{AcpAgent, Agent, ConnectionTo};
use clap::Parser;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Parser)]
struct Cli {
    /// Shell-style command that starts the ACP agent (e.g. "node .../codex-acp/dist/index.js").
    #[arg(long)]
    agent_cmd: String,
    /// Working directory for the child session.
    #[arg(long)]
    cwd: PathBuf,
    /// Prompt to send.
    #[arg(long)]
    prompt: String,
    /// Directory where events.jsonl and result.json are written.
    #[arg(long)]
    out: PathBuf,
    /// KEY=VALUE environment variables for the agent process.
    #[arg(long = "env")]
    envs: Vec<String>,
    /// JSON object sent as `_meta` on session/new (bridge-specific native options).
    #[arg(long)]
    session_meta: Option<String>,
}

#[derive(Default)]
struct Captured {
    text: String,
    usage_updates: Vec<serde_json::Value>,
    tool_calls: usize,
    updates: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.out)?;
    let events = Arc::new(Mutex::new(File::create(cli.out.join("events.jsonl"))?));
    let captured = Arc::new(Mutex::new(Captured::default()));
    let t0 = Instant::now();

    let mut config = AcpAgent::from_str(&cli.agent_cmd)?.into_config();
    for kv in &cli.envs {
        let (k, v) = kv.split_once('=').ok_or("--env expects KEY=VALUE")?;
        config = config.env(k, v);
    }
    let agent = AcpAgent::new(config);

    let ev = events.clone();
    let cap = captured.clone();
    let ev2 = events.clone();
    let cwd = cli.cwd.clone();
    let prompt = cli.prompt.clone();
    let out = cli.out.clone();
    let session_meta: Option<serde_json::Map<String, serde_json::Value>> = match &cli.session_meta {
        Some(m) => Some(serde_json::from_str(m)?),
        None => None,
    };

    agent_client_protocol::Client
        .builder()
        .name("sub-acp-spike")
        .on_receive_notification(
            async move |n: SessionNotification, _cx| {
                let ms = t0.elapsed().as_millis();
                let mut c = cap.lock().unwrap();
                c.updates += 1;
                match &n.update {
                    SessionUpdate::AgentMessageChunk(chunk) => {
                        if let ContentBlock::Text(t) = &chunk.content {
                            c.text.push_str(&t.text);
                        }
                    }
                    SessionUpdate::ToolCall(_) => c.tool_calls += 1,
                    SessionUpdate::UsageUpdate(u) => {
                        c.usage_updates.push(serde_json::to_value(u).unwrap_or_default())
                    }
                    _ => {}
                }
                let rec = serde_json::json!({"t_ms": ms, "kind": "session/update", "notification": n});
                writeln!(ev.lock().unwrap(), "{}", rec).ok();
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |req: RequestPermissionRequest, responder, _cx| {
                let ms = t0.elapsed().as_millis();
                let rec = serde_json::json!({"t_ms": ms, "kind": "session/request_permission", "request": req});
                writeln!(ev2.lock().unwrap(), "{}", rec).ok();
                // Auto-approve: pick the first "allow" option, else the first option.
                let pick = req
                    .options
                    .iter()
                    .find(|o| format!("{:?}", o.kind).to_lowercase().contains("allow"))
                    .or(req.options.first())
                    .map(|o| o.option_id.clone());
                match pick {
                    Some(id) => responder.respond(RequestPermissionResponse::new(
                        RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(id)),
                    )),
                    None => responder.respond(RequestPermissionResponse::new(
                        RequestPermissionOutcome::Cancelled,
                    )),
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(agent, |cx: ConnectionTo<Agent>| async move {
            let init = cx
                .send_request(InitializeRequest::new(ProtocolVersion::V1))
                .block_task()
                .await?;
            eprintln!("initialized: {}", serde_json::to_string(&init).unwrap_or_default());
            let new = cx
                .send_request(NewSessionRequest::new(cwd.clone()).meta(session_meta))
                .block_task()
                .await?;
            let session_id = new.session_id.clone();
            eprintln!("handle (session id): {}", session_id);
            let launched_ms = t0.elapsed().as_millis();
            let resp = cx
                .send_request(PromptRequest::new(
                    session_id.clone(),
                    vec![ContentBlock::Text(TextContent::new(prompt))],
                ))
                .block_task()
                .await?;
            let finished_ms = t0.elapsed().as_millis();
            let c = captured.lock().unwrap();
            let result = serde_json::json!({
                "agent_info": init.agent_info,
                "agent_capabilities": init.agent_capabilities,
                "handle": {"session_id": session_id, "cwd": cwd},
                "launched_ms": launched_ms,
                "finished_ms": finished_ms,
                "stop_reason": resp.stop_reason,
                "prompt_response_raw": resp,
                "updates": c.updates,
                "tool_calls": c.tool_calls,
                "usage_updates": c.usage_updates,
                "final_text": c.text,
            });
            std::fs::write(out.join("result.json"), serde_json::to_string_pretty(&result)?)
                .map_err(|e| agent_client_protocol::Error::from(anyhow::Error::from(e)))?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        })
        .await?;
    Ok(())
}
