# Architecture: crate layout

`sub` is one Cargo workspace. The SDK is the kernel; every other crate consumes it. Rationale: [`decisions/2026-08-27-workspace-layout.md`](decisions/2026-08-27-workspace-layout.md).

| Crate | Kind | Role |
|:--|:--|:--|
| `sub-sdk` | library | The kernel: the delegated-work model and the SDK. Owns the shared ACP client layer (`sub_sdk::acp`) and depends directly on the ACP SDK crate. |
| `sub-cli` | binary `sub` | Command-line surface for humans: bridge install, launch, wait, list, and inspect. |
| `sub-mcp` | binary `sub-mcp` | MCP stdio server for agents: bridge install, launch, wait, list, and inspect. |
| `sub-harness-fake` | library and binary `sub-harness-fake` | Programmable fake harness (ACP v1 over stdio) for the behavioral contract suite. Owns fixture loading, scenario scripting, and the replay server. Depends directly on `sub-sdk` and the ACP SDK crate. |
| `sub-adapter-claude` | library | Adapter for `claude`, through pinned `@agentclientprotocol/claude-agent-acp` 0.70.0. |
| `sub-adapter-codex` | library | Adapter for `codex`, through pinned `@agentclientprotocol/codex-acp` 1.6.2. |
| `sub-adapter-cursor` | library | Adapter for `cursor-agent`, which speaks ACP natively. |

Dependency direction: surfaces → adapters and `sub-sdk` → `agent-client-protocol`, `tokio`; `sub-harness-fake` → `sub-sdk`, `agent-client-protocol`, `tokio`. Only `sub-sdk` and the programmable fake depend directly on ACP types. Adapters construct SDK launch data and never depend on a surface.

The shared ACP client layer in `sub-sdk` spawns an agent process, negotiates protocol v1, opens a session with adapter metadata, sets the requested harness-native permission mode and optional model, sends a prompt, consumes the update stream, captures unstable per-turn token usage, denies permission requests, cancels, and times out. The delegated-work kernel writes task/attempt state and a typed normalized event log, launches one independent supervisor per attempt, accumulates reported usage, derives a bounded result, implements repeatable wait, and provides read-only list/inspect operations over the state directory. The fake harness in `sub-harness-fake` replays fixtures with and without usage and accepts the same mode/model controls; the contract suite drives fake and real harnesses through the same client API.

Claude and Codex launch, wait, and Observe are implemented across the SDK, CLI, and MCP. `sub list` / `sub_list` return compact task status and usage; `sub inspect` / `sub_inspect` return the same task data plus attempt state and normalized events. These reads require only the state directory and work while a detached supervisor runs or after it exits. Events contain task/attempt linkage, lifecycle, coalesced activity categories, and accumulated reported usage; they never copy transcript content. The vendor-native session record remains raw evidence by reference. Claude reports cost and tokens, Codex reports tokens, and unsupported measurements remain null beside explicit `usage_support` flags. Rationale: [`decisions/2026-09-01-observe-events-and-usage.md`](decisions/2026-09-01-observe-events-and-usage.md).

Public shapes are proposed in pull requests; no `sub.toml` is required for the beta path because every required launch value and state directory can be supplied explicitly.

Pinned dependencies are declared once in the root `Cargo.toml` under `[workspace.dependencies]` with exact (`=`) versions; crates reference them with `.workspace = true`. Lints are also workspace-wide (`[workspace.lints]`).
