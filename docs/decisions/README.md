# Decision records

One file per repository-level decision, named `YYYY-MM-DD-<slug>.md`. Each states the decision, a one-paragraph rationale, and what would revisit it. These cover only what the mental model delegates to the repository (crate layout, async runtime, ACP SDK choice, CI configuration, release tooling, docs skeleton, and the like). Product and public-interface decisions are the mental model's and are not recorded here.

| Date | Decision |
|:--|:--|
| 2026-08-27 | [Workspace layout](2026-08-27-workspace-layout.md) |
| 2026-08-27 | [Async runtime: tokio](2026-08-27-async-runtime.md) |
| 2026-08-27 | [ACP SDK: `agent-client-protocol` 2.0.0](2026-08-27-acp-sdk.md) |
| 2026-08-27 | [Toolchain and tool pinning](2026-08-27-toolchain-pinning.md) |
| 2026-08-27 | [Verification entry point and gates](2026-08-27-verification-entry-point.md) |
| 2026-08-27 | [Coverage tool: cargo-llvm-cov](2026-08-27-coverage-tool.md) |
| 2026-08-27 | [Dependency audit: cargo-deny](2026-08-27-dependency-audit.md) |
| 2026-08-27 | [CI: GitHub Actions layout](2026-08-27-ci-layout.md) |
| 2026-08-27 | [Harness configuration](2026-08-27-harness-configuration.md) |
| 2026-08-27 | [Bridge pinning and installation](2026-08-27-bridge-pinning.md) |
| 2026-08-27 | [Docs skeleton](2026-08-27-docs-skeleton.md) |
| 2026-08-27 | [Release tooling: cargo-dist](2026-08-27-release-tooling.md) |
| 2026-08-31 | [Fake harness fixture format](2026-08-31-fake-harness-fixtures.md) |
| 2026-08-31 | [Fake harness scenario scripting](2026-08-31-fake-harness-scenarios.md) |
| 2026-08-31 | [Delegation kernel, durable handles, results, and adapter side channels](2026-08-31-delegation-kernel-and-adapters.md) |
| 2026-09-01 | [Observe event vocabulary and usage accumulation](2026-09-01-observe-events-and-usage.md) |
| 2026-09-01 | [Supervisor liveness and orphaned attempt status](2026-09-01-orphan-detection-and-status.md) |
| 2026-09-01 | [Recovery attempts and bridge session resume](2026-09-01-recovery-resume-mechanics.md) |
| 2026-09-01 | [Cross-process cancel request signalling](2026-09-01-cancel-request-signalling.md) |
| 2026-09-01 | [Cancellation grace period and ignored harnesses](2026-09-01-cancel-grace-and-ignored-harness.md) |
| 2026-09-01 | [`sub.toml` location and launch precedence](2026-09-01-sub-toml-location-and-precedence.md) |
| 2026-09-01 | [Onboarding installation for Claude and Codex](2026-09-01-onboarding-installation.md) |
| 2026-09-01 | [Cursor native ACP transport and extension handling](2026-09-01-cursor-native-acp-and-extensions.md) |
| 2026-09-01 | [Load-replay observation boundary](2026-09-01-load-replay-observation-boundary.md) |
| 2026-09-02 | [CodeScene-scoped commit and pull-request verification gates](2026-09-02-verification-gates.md) |
