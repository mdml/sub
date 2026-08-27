# Architecture: crate layout

`sub` is one Cargo workspace. The SDK is the kernel; every other crate consumes it. Rationale: [`decisions/2026-08-27-workspace-layout.md`](decisions/2026-08-27-workspace-layout.md).

| Crate | Kind | Role |
|:--|:--|:--|
| `sub-sdk` | library | The kernel: the delegated-work model and the SDK. The only crate that depends on the ACP SDK crate directly. |
| `sub-cli` | binary `sub` | Command-line surface for humans. Depends on `sub-sdk`. No commands yet. |
| `sub-mcp` | binary `sub-mcp` | MCP server surface for agents. Depends on `sub-sdk`. No tools yet. |
| `sub-harness-fake` | library | The programmable fake harness that the behavioral contract suite runs against. Depends on `sub-sdk`. |
| `sub-adapter-claude` | library | Adapter for `claude`, through its ACP bridge. |
| `sub-adapter-codex` | library | Adapter for `codex`, through its ACP bridge. |
| `sub-adapter-cursor` | library | Adapter for `cursor-agent`, which speaks ACP natively. |

Dependency direction: surfaces and adapters → `sub-sdk` → `agent-client-protocol`, `tokio`. Adapters never depend on surfaces; surfaces never depend on adapters directly (the SDK will select adapters).

Every crate is a stub: it compiles, carries a doc comment stating its role, and has a test so that coverage is measured from the first commit. Public shapes (SDK types, MCP tool names, CLI commands, result/event/params shapes, the `sub.toml` schema) are not defined; the mental model says how they are proposed.

Pinned dependencies are declared once in the root `Cargo.toml` under `[workspace.dependencies]` with exact (`=`) versions; crates reference them with `.workspace = true`. Lints are also workspace-wide (`[workspace.lints]`).
