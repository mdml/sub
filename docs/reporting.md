# Reporting problems

GitHub issue forms are the intake for bug reports and feature requests. A bug report records the `sub` version or release tag, manager and child harness versions, the bounded task, expected and actual behavior, the task handle, and scrubbed `sub inspect` output.

Run `sub report <handle>` when a delegated task suggests that `sub` itself misbehaved. The command reads durable task evidence and prints a ready-to-run `gh issue create -R mdml/sub` command. It never invokes `gh`, submits an issue, reads credentials, copies a native transcript, or writes state. Review and edit its command before running it.

The draft keeps the task handle, child harness, `sub` version, the first prompt line, and normalized `sub inspect` evidence. It replaces the known home directory and hostname and excludes later prompt lines, result summaries, native-session references, supervisor logs, and other state fields known to contain user content. Automated scrubbing is not a guarantee: redact any remaining home paths, prompt content, hostnames, credentials, customer data, or project details before submission.

The manager's delegation skill tells the manager to capture the handle, run `sub report <handle>`, and hand the resulting command to the user. The manager never files the issue itself.

## Labels and stable candidates

Use `harness:claude`, `harness:codex`, or `harness:cursor` to identify the child integration involved. Use `regression` only when behavior worked in an earlier release.

`stable-candidate` means that the commits from a merged PR are proposed as named fixes for the next assembled stable pointer. It is not a promise that the change will ship. When the stable promotion dispatch has no explicit cherry-pick list, it finds merged PRs with this label, keeps their commits that are in `main` after the selected nightly base, orders them as they appear in `main`, and shows that default in the preview summary for confirmation.
