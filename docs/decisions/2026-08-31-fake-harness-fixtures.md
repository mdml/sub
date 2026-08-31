# Fake harness fixture format

Date: 2026-08-31. Status: adopted.

## Decision

Each fixture is a directory containing `fixture.toml` (manifest) and a JSONL event stream file named by the manifest's top-level `events` key. The manifest records provenance (`source.kind = "recorded"` with harness name and version, or `"synthetic"`), agent identity for `initialize`, default `session_id`, prompt defaults (`stop_reason`, optional `replay_timing`), and the events filename. Event lines follow the spike capture shape: `{ "t_ms", "kind", "notification" }` where `kind` is typically `session/update` and `notification` is a serialized ACP `SessionNotification`. Top-level manifest keys must appear before any `[table]` header in TOML so they are not parsed into the wrong table.

## Rationale

The spike already captured streams in this JSONL shape; reusing it avoids a conversion step and keeps the fake harness aligned with what adapters will see on the wire. TOML keeps manifests human-editable and distinct from the large JSONL payload. Stamping recorded fixtures with harness name and version supports the mental model's requirement that fixtures declare what they were recorded from; synthetic fixtures use `source.kind = "synthetic"` instead.

## Revisit when

Fixtures need wire-level JSON-RPC frames rather than session notifications, or ACP v2 changes the notification shape.
