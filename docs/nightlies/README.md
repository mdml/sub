# On-machine nightlies

The harness-compatibility nightly runs on a maintainer machine, not on GitHub, because it needs the real installed harnesses. `scripts/nightly/harness-compatibility.sh` is scheduled outside the repository, records installed and adapter-declared versions, and runs the behavioral contract suite against each available real harness. It writes a dated report and exits non-zero when a real-harness contract fails.

| Job | Definition | Script | Needs |
|:--|:--|:--|:--|
| Harness compatibility | [`harness-compatibility.md`](harness-compatibility.md) | `scripts/nightly/harness-compatibility.sh` | Installed `claude`, `codex`, `cursor-agent`; previously installed Claude and Codex bridge paths; the behavioral contract suite. |

The GitHub nightly (`.github/workflows/nightly.yml`) is separate and covers vulnerability and dependency-freshness checks.
