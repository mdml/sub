# On-machine nightlies

The harness-compatibility nightly runs on a maintainer machine, not on GitHub, because it needs the real installed harnesses. Its documented script under `scripts/nightly/` is scheduled outside the repository. It is not implemented beyond its definition yet; the script states what it will do and exits non-zero until the contract suite exists.

| Job | Definition | Script | Needs |
|:--|:--|:--|:--|
| Harness compatibility | [`harness-compatibility.md`](harness-compatibility.md) | `scripts/nightly/harness-compatibility.sh` | Installed `claude`, `codex`, `cursor-agent`; the contract suite (next item in the mental model's "Next for agents"). |

The GitHub nightly (`.github/workflows/nightly.yml`) is separate and covers vulnerability and dependency-freshness checks.
