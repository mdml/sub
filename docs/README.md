# `sub` documentation

These documents describe the current state of the repository on `staging` and `main`. They are updated in the same pull request as the code they describe.

Product intent, decided boundaries, beta scope, and the proofs that gate releases live in the owner's mental model, which reaches agents as the `sub-mental-model` skill (see [`AGENTS.md`](../AGENTS.md)). Nothing here duplicates it; where a document needs one of its decisions, it names the mental model.

| Document | Purpose |
|:--|:--|
| [`architecture.md`](architecture.md) | Crate layout and how the crates depend on each other. |
| [`verification.md`](verification.md) | The verification entry point, the two gates, and what CI runs. |
| [`harnesses.md`](harnesses.md) | How `claude`, `codex`, and `cursor-agent` are configured to develop this repository. |
| [`decisions/`](decisions/README.md) | Decision records: one file per repository-level decision, dated. |
| [`nightlies/`](nightlies/README.md) | The project-specific harness-compatibility nightly definition. |
| [`spikes/`](spikes/) | Spike reports. Evidence only; the mental model says what was decided. |
| [`proofs/`](proofs/) | Re-runnable feature-proof scenarios; captured evidence lives separately under the repository's top-level `proofs/`. |
| [`scenarios/`](scenarios/) | Re-runnable non-gating product scenarios; scrubbed evidence lives under the repository's top-level `scenarios/`. |
| [`release.md`](release.md) | How releases will be built, signed, and published. |
