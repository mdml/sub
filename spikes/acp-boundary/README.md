# ACP boundary spike — evidence

**This directory is evidence only** for `docs/spikes/acp-boundary.md`. It is not wired into any workspace, build, CI, or verification entry point, and nothing in `sub` may depend on it.

## Layout

- `evidence/` — captured output from the runs described in the spike report, plus the raw JSON-RPC probe scripts used for the recover mapping.
- `proto/` — the disposable prototype (`acp-delegate`, a standalone Cargo crate acting as a one-shot ACP client: spawn an agent process, open one session, send one prompt, auto-approve every `session/request_permission`, write every `session/update` to `events.jsonl` and a `result.json`). Deleted when the spike was resolved on 2026-08-27; recover it with `git log -- spikes/acp-boundary/proto` to rerun.

## Rerun (requires the deleted `proto/`)

Prerequisites: `cargo`, `node`/`npm`, and the harnesses' existing logins. Nothing here writes credentials or changes harness configuration.

```bash
# 1. Bridges (pin the versions the report was written against)
mkdir -p /tmp/acp-bridges && cd /tmp/acp-bridges && npm init -y >/dev/null
npm install @agentclientprotocol/codex-acp@1.6.2 @agentclientprotocol/claude-agent-acp@0.70.0

# 2. Build the client
cd <repo>/spikes/acp-boundary/proto && cargo build

# 3. Delegate to codex in an empty directory (subagent flag passed via CODEX_CONFIG; see report for whether it took effect)
mkdir -p /tmp/acp-run/work /tmp/acp-run/out
env -u CLAUDECODE CODEX_PATH="$(which codex)" CODEX_CONFIG='{"features":{"multi_agent":false}}' \
  target/debug/acp-delegate \
  --agent-cmd "node /tmp/acp-bridges/node_modules/@agentclientprotocol/codex-acp/dist/index.js" \
  --cwd /tmp/acp-run/work --out /tmp/acp-run/out \
  --prompt 'In this empty directory, create a hello-world Rust crate (binary) with one unit test. Run `cargo test` and report the full output of that command in your final message.'

# claude child (subagents disabled through the bridge's native-options passthrough)
env -u CLAUDECODE target/debug/acp-delegate --agent-cmd /tmp/acp-bridges/node_modules/.bin/claude-agent-acp \
  --session-meta '{"claudeCode":{"options":{"disallowedTools":["Task","Agent"]}}}' --cwd ... --out ... --prompt ...

# cursor child (native ACP server; no subagent switch found)
env -u CLAUDECODE target/debug/acp-delegate --agent-cmd "cursor-agent acp" --cwd ... --out ... --prompt ...
```

`env -u CLAUDECODE` is needed only when launching from inside a Claude Code session; the claude bridge refuses to start nested otherwise.

Recover probes (raw JSON-RPC, no SDK): `python3 evidence/resume_probe.py <session-id> <cwd> <agent command...>` sends `session/resume` in a fresh agent process and asks the child what it did earlier; `evidence/cursor_load_probe.py` does the same with `session/load`.
