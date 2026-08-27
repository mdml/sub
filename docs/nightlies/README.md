# On-machine nightlies

Two nightly jobs run on the owner's machine, not on GitHub, because they need the real installed harnesses and the owner's Dogtag MCP. Each is a documented script under `scripts/nightly/` that the owner schedules (cron, `launchd`, or a harness's own scheduler). Neither is implemented beyond its definition yet; each script states what it will do and exits non-zero until the pieces it needs exist.

| Job | Definition | Script | Needs |
|:--|:--|:--|:--|
| Harness compatibility | [`harness-compatibility.md`](harness-compatibility.md) | `scripts/nightly/harness-compatibility.sh` | Installed `claude`, `codex`, `cursor-agent`; the contract suite (next item in the mental model's "Next for agents"). |
| Mental-model audit | [`mental-model-audit.md`](mental-model-audit.md) | `scripts/nightly/mental-model-audit.sh` | A harness with the Dogtag MCP and the `sub-mental-model` skill. |

The GitHub nightly (`.github/workflows/nightly.yml`) is separate and covers vulnerability and dependency-freshness checks.
