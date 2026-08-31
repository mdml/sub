# Architecture: crate layout

`sub` is one Cargo workspace. The SDK is the kernel; every other crate consumes it. Rationale: [`decisions/2026-08-27-workspace-layout.md`](decisions/2026-08-27-workspace-layout.md).

| Crate | Kind | Role |
|:--|:--|:--|
| `sub-sdk` | library | The kernel: the delegated-work model and the SDK. Owns the shared ACP client layer (`sub_sdk::acp`) and depends directly on the ACP SDK crate. |
| `sub-cli` | binary `sub` | Command-line surface for humans. Depends on `sub-sdk`. No commands yet. |
| `sub-mcp` | binary `sub-mcp` | MCP server surface for agents. Depends on `sub-sdk`. No tools yet. |
| `sub-harness-fake` | library and binary `sub-harness-fake` | Programmable fake harness (ACP v1 over stdio) for the behavioral contract suite. Owns fixture loading, scenario scripting, and the replay server. Depends directly on `sub-sdk` and the ACP SDK crate. |
| `sub-adapter-claude` | library | Adapter for `claude`, through its ACP bridge. |
| `sub-adapter-codex` | library | Adapter for `codex`, through its ACP bridge. |
| `sub-adapter-cursor` | library | Adapter for `cursor-agent`, which speaks ACP natively. |

Dependency direction: surfaces and adapters → `sub-sdk` → `agent-client-protocol`, `tokio`; `sub-harness-fake` → `sub-sdk`, `agent-client-protocol`, `tokio`. Adapters and surfaces never depend directly on ACP types. Adapters never depend on surfaces; surfaces never depend on adapters directly (the SDK will select adapters).

The shared ACP client layer in `sub-sdk` spawns an agent process, negotiates protocol v1, opens a session, sends a prompt, consumes the update stream, denies permission requests, cancels, and times out. The fake harness in `sub-harness-fake` is a child process that replays fixture streams and is scriptable per scenario; its library exposes the fixture, scenario, and replay-server implementation to tests without placing test scaffolding in the kernel. The contract suite in `sub-sdk` drives fake and real harnesses through the same client API (fake harness in CI; real harnesses opt in via `SUB_CONTRACT_REAL_HARNESS`).

Every crate except the adapters and surfaces listed above is implemented for the test boundary. Adapter and surface crates remain stubs. Public shapes (SDK types, MCP tool names, CLI commands, result/event/params shapes, the `sub.toml` schema) are proposed in pull requests; the ACP client types in `sub_sdk::acp` are part of that proposal.

Pinned dependencies are declared once in the root `Cargo.toml` under `[workspace.dependencies]` with exact (`=`) versions; crates reference them with `.workspace = true`. Lints are also workspace-wide (`[workspace.lints]`).
