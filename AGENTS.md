# Agent instructions

These instructions apply to every coding agent working in this repository (`claude`, `codex`, `cursor-agent`). `CLAUDE.md` imports this file; do not duplicate content between the two.

## The mental model

`sub` has a human-owned mental model that lives outside this repository. It states product intent, the decisions already made, the hypotheses under test, the active spikes, and the proofs that gate releases. This repository describes the implemented system; it does not duplicate the mental model.

- It reaches you as a skill named `sub-mental-model`, loaded from the owner's user scope in every `claude`, `codex`, and `cursor-agent` session. Invoke it before any product or public-interface decision — SDK types, MCP tool names, CLI commands, result/event/params shapes, the `sub.toml` schema, naming, scope — and read the model in full.
- If the `sub-mental-model` skill is absent from your session, stop and ask for the mental model before making any such decision. Do not reconstruct it from this repository or proceed on inference.
- Never edit the mental model. Propose changes in your handoff instead.

## Assumption boundary

Assume the owner knows only what is in the current session and the mental model. Repository documents are authoritative about the implemented system, but their contents are not part of the owner's working memory. When a task needs a concept that is in neither place, name it before proceeding.

## When to stop

Stop and return the decision to the owner when:

- The implementation would conflict with the mental model.
- The task would freeze a product or public-interface decision the mental model does not represent, other than a shape the mental model says to propose in a PR.
- A sequence of locally reasonable changes is moving the project's main ideas.
- You are inferring product intent from implementation.

The mental model lists what agents decide without asking. Decide those, document them here, and move on.

## Handoff

Reload the relevant part of the mental model before reporting. Explain what changed in the mental model's vocabulary, give one worked example when behavior changed, and report any new gap between the mental model and the repository as a candidate change to the mental model, not an edit.

## Process

- Work in your own git worktree and open development PRs into `staging`. Rebase-merge those PRs to keep development history linear. Conventional Commits.
- Promote `staging` into `main` with a reviewed, gated promotion PR merged by merge commit. Promotion is triggered per proof or milestone, not batched.
- `main` is the nightly channel. Its ruleset admits promotion PR merge commits but rejects direct pushes, force-pushes, and deletion without bypass.
- `stable` is assembled by the `promote to stable` workflow from a nightly-tagged `main` commit plus explicitly ordered cherry-picks. Its ruleset permits only holders of the release deploy key to create, update, or delete the branch; the private key lives only in this repository's Actions secrets and humans have no bypass.
- Humans never create release tags. The nightly and stable promotion workflows create tags over SSH with the release deploy key, and the release-tag ruleset permits only holders of that key to write matching tags.
- Documentation describes the current state of the repository on `staging` and `main`. Update it in the same PR as the code it describes.
- Spikes land in `docs/spikes/<name>.md`, with captured evidence and any disposable prototype under `spikes/<name>/`. Run a spike only when the task names it. A resolved spike keeps its report and evidence; its prototype is deleted at resolution and stays recoverable from git history.
- Resolved spikes: ACP boundary (`docs/spikes/acp-boundary.md`, 2026-08-27). Its recommendation is adopted in the mental model; read the mental model, not the report, for what was decided.
- The verification entry point is `scripts/verify.sh` (`just verify`); stage the intended changes, run it before every commit, and say in your handoff that you did. The pull-request gate is `scripts/verify.sh --full`, relative to `origin/staging` by default. See `docs/verification.md`.
