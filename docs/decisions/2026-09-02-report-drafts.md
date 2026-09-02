# Scrubbed issue-report drafts

Date: 2026-09-02. Status: proposed public shape implemented for review.

## Decision

`sub report <handle>` is a CLI-only public shape. It reads one durable task and emits a shell-quoted `gh issue create -R mdml/sub` command; it never runs `gh`, submits an issue, reads credentials, copies a native harness record, or writes task state. Reporting is not a delegated-work control, and the mental model assigns issue drafting to the human CLI, so there is no MCP mirror.

The draft includes `sub`'s version, the task handle, child harness, first line of the delegated prompt, and serialized normalized `sub inspect` evidence. It replaces the current home directory and hostname with markers. It excludes later prompt lines, result summaries, artifact locations, native transcripts, supervisor logs, adapter launch data, harness binary paths, working directories, and all other persisted fields known to contain user content. The command help and generated body both warn the user to scrub remaining home paths, prompt content, hostnames, credentials, and other user content.

The installed delegation skill adds one failure path: capture the handle, run `sub report <handle>`, and hand the command to the user. The manager never files the issue.

## Rationale

Normalized inspection evidence contains lifecycle and usage facts without reconstructing the child transcript. Keeping only the prompt's first line supplies enough identity for the human to recognize the delegated work while sharply reducing accidental disclosure. A draft command is reviewable and composable without giving `sub` credential or issue-submission responsibility.

## Revisit when

Revisit if task state gains a typed privacy classification, harness versions become durable normalized evidence, GitHub supports prefilled custom issue-form fields in URLs, or user reports show that the first prompt line is either insufficient or too sensitive by default.
