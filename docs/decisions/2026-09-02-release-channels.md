# Nightly and stable release mechanics

Date: 2026-09-02. Status: adopted.

## Decision

The development channel is `staging`, the nightly channel is `main`, and the stable channel is the workflow-owned `stable` branch. Promotion from `staging` to `main` is a gated PR merged with a merge commit per proof or milestone. Humans do not tag releases.

The nightly workflow publishes outside cargo-dist's tag machinery. It skips an unchanged `main`, runs the full gate with whole-tree CodeScene, builds and attests native archives for every supported target, creates an annotated `v<workspace-version>-nightly.<UTC-date>.<run-number>.<attempt>` tag at the verified `main` tip, and creates a GitHub prerelease with checksums. It does not publish Homebrew. This avoids changing the workspace version on `main`: cargo-dist 0.32 rejects `v0.0.0-nightly.20260902.1` when workspace packages are version `0.0.0`.

Stable promotion is a two-phase `workflow_dispatch`. A preview resolves the base and ordered commits and writes them to the run summary; only a second run with `confirm` enabled mutates refs. Blank cherry-picks default to commits from merged `stable-candidate` PRs that are in `main` after the base, in `main` order. An explicit list replaces the default. The base must be a nightly tag in `main` unless `allow_untagged_base` is deliberately enabled. Cherry-pick conflicts and release-prep reconciliation conflicts halt without resolution or remote mutation.

During 0.x, an auto-derived stable version increments minor for either a breaking Conventional Commit or `feat`, and patch otherwise. An exact advancing `0.x.y` override is accepted. Release prep cuts the Unreleased changelog, updates the workspace version and internal exact dependency versions, refreshes `Cargo.lock`, and passes the full whole-tree gate. The same release-prep commit must cherry-pick onto current `staging`; the workflow atomically pushes that reconciliation branch with `stable` and the stable tag, then opens a small PR into `staging`.

cargo-dist remains the stable artifact publisher. `dispatch-releases = true` lets promotion explicitly dispatch the generated `release.yml` after creating a tag with `GITHUB_TOKEN`; this avoids GitHub's suppression of tag-push workflow recursion. `publish-prereleases = false` keeps Homebrew stable-only. The generated workflow is regenerated, never hand-edited.

## Rulesets

GitHub's current repository-ruleset API requires an integration bypass as `{ "actor_id": 15368, "actor_type": "Integration", "bypass_mode": "always" }`. `gh api /apps/github-actions` identifies app ID 15368 as `github-actions`, and the [create-ruleset API](https://docs.github.com/en/rest/repos/rules?apiVersion=2022-11-28#create-a-repository-ruleset) defines `Integration` and `always` for this payload. A ruleset cannot narrow an integration bypass to one workflow file; the repository therefore grants write permission only to workflows that need it, and only `promote-stable.yml` contains a `stable` branch update.

The owner must create the stable ruleset before the branch exists:

```sh
gh api --method POST repos/mdml/sub/rulesets --input - <<'JSON'
{
  "name": "stable",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [
    {"actor_id": 15368, "actor_type": "Integration", "bypass_mode": "always"}
  ],
  "conditions": {
    "ref_name": {"include": ["refs/heads/stable"], "exclude": []}
  },
  "rules": [
    {"type": "creation"},
    {"type": "update"},
    {"type": "deletion"},
    {"type": "non_fast_forward"}
  ]
}
JSON
```

The owner must also create a tag ruleset so only workflows can create, move, or delete release tags:

```sh
gh api --method POST repos/mdml/sub/rulesets --input - <<'JSON'
{
  "name": "release tags",
  "target": "tag",
  "enforcement": "active",
  "bypass_actors": [
    {"actor_id": 15368, "actor_type": "Integration", "bypass_mode": "always"}
  ],
  "conditions": {
    "ref_name": {"include": ["refs/tags/v*"], "exclude": []}
  },
  "rules": [
    {"type": "creation"},
    {"type": "update"},
    {"type": "deletion"}
  ]
}
JSON
```

No `main` ruleset update is needed. The current rule requires `verify-full`, permits only merge-commit PRs, and blocks deletion and non-fast-forward updates without bypass.

## Rationale

The design keeps `main` equal to the dogfooded source that was verified, makes stable composition explicit and reviewable, and prevents release preparation from becoming an untracked divergence. The preview/confirm split is the smallest auditable confirmation mechanism available to `workflow_dispatch`. Whole-tree CodeScene in nightly and stable gates is the standing backstop for health drift in unchanged files.

## Revisit when

Revisit if cargo-dist accepts a prerelease release tag without requiring a matching prerelease workspace version, GitHub rulesets gain workflow-file-scoped bypasses, candidate discovery approaches the 100-PR query limit, or stable composition needs more than conflict-free cherry-picks.
