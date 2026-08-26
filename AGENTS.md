# Agent instructions

These instructions apply to every coding agent working in this repository (`claude`, `codex`, `cursor-agent`). `CLAUDE.md` imports this file; do not duplicate content between the two.

## The mental model

`sub` has a human-owned mental model that lives outside this repository. It states product intent, the decisions already made, the hypotheses under test, the active spikes, and the proofs that gate releases. This repository describes the implemented system; it does not duplicate the mental model.

- Load it through the user-level pointer named `sub-mental-model` (a skill, or a line in your user-level instructions). Read it in full before any product or public-interface decision.
- If the pointer is absent from your environment, stop and ask for the mental model before making any product or public-interface decision. Do not reconstruct it from this repository.
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

- Work in your own git worktree. Open a PR into `staging`. `main` is promoted from `staging` by the owner, per proof.
- Rebase-merge only; linear history. Conventional Commits.
- Documentation describes the current state of the repository on `staging` and `main`. Update it in the same PR as the code it describes.
- Spikes land in `docs/spikes/<name>.md`. Run a spike only when the task names it.
- One verification entry point will exist for all three harnesses; until it does, say in your handoff what you ran.
