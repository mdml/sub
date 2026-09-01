# Scenario: configure, onboard, and delegate without explicit binary arguments

This non-gating product scenario demonstrates the beta onboarding path from an empty throwaway root. It was executed on 2026-09-01 with Claude Code 2.1.252, Codex CLI 0.151.0, `@agentclientprotocol/claude-agent-acp` 0.70.0, and `@agentclientprotocol/codex-acp` 1.6.2. Unlike the feature proofs, this scenario records setup usability and does not gate a release.

## Isolation

Create an empty throwaway `$ROOT`, with `$WORK`, `$STATE_DIR`, `$CLAUDE_ROOT`, and `$CODEX_ROOT` beneath it. Set every override below for both onboarding runs. These values prevent writes to real Claude or Codex skills and global configuration:

```sh
export SUB_CONFIG="$ROOT/sub.toml"
export SUB_CLAUDE_CONFIG="$CLAUDE_ROOT/config.json"
export SUB_CLAUDE_SKILLS_DIR="$CLAUDE_ROOT/skills"
export SUB_CODEX_CONFIG="$CODEX_ROOT/config.toml"
export SUB_CODEX_SKILLS_DIR="$CODEX_ROOT/skills"
export SUB_MCP_BINARY="$SUB_MCP_BIN"
```

`SUB_MCP_BINARY` is an isolation/test override; a normal installation finds `sub-mcp` beside `sub`. The real child launch uses the harness's existing authentication. `sub` neither receives nor stores credentials.

## Configuration and onboarding

Write this beta-minimum `$SUB_CONFIG`, substituting absolute throwaway and installed-binary paths:

```toml
state_dir = "$STATE_DIR"

[harnesses.claude]
binary = "$CLAUDE_BIN"
permission_mode = "bypassPermissions"

[harnesses.codex]
binary = "$CODEX_BIN"
permission_mode = "agent"
```

Run the one onboarding action twice:

```sh
sub onboard claude codex
sub onboard claude codex
```

The first run reports installed bridges and created skills and MCP registrations. The second reports every component unchanged. The resulting throwaway layout contains only the two requested harness roots, the selected state directory, and `sub.toml`. Scrubbed reports and layout are in [`../../scenarios/onboarding/evidence/`](../../scenarios/onboarding/evidence/).

## Config-only launch and wait

Launch one real Codex task without `--binary`, `--permission-mode`, or `--state-dir`, then wait without a state argument:

```sh
sub launch --harness codex --cwd "$WORK" --prompt 'Create onboarding.txt containing exactly onboarding config passed, then report the changed file using a Markdown link.'
sub wait "$HANDLE" --timeout-seconds 120
```

Launch returns a task handle. Wait returns `succeeded`, reports `$WORK/onboarding.txt`, and retains the native Codex session by reference; the file contains exactly `onboarding config passed`. The same checkout's opt-in real-harness contract mode passed against Claude and Codex after this run. Captured evidence includes the launch, wait, output file, installed versions, and contract results.
