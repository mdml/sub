# Nightly and stable release mechanics

Date: 2026-09-02. Status: adopted.

## Decision

The development channel is `staging`, the nightly channel is `main`, and the stable channel is the workflow-owned `stable` branch. Promotion from `staging` to `main` is a gated PR merged with a merge commit per proof or milestone. Humans do not tag releases.

The nightly workflow publishes outside cargo-dist's tag machinery. It skips an unchanged `main`, runs the full gate with whole-tree CodeScene, builds and attests native archives for every supported target, creates an annotated `v<workspace-version>-nightly.<UTC-date>.<run-number>.<attempt>` tag at the verified `main` tip, and creates a GitHub prerelease with checksums. It does not publish Homebrew. This avoids changing the workspace version on `main`: cargo-dist 0.32 rejects `v0.0.0-nightly.20260902.1` when workspace packages are version `0.0.0`.

Stable promotion is a two-phase `workflow_dispatch`. A preview resolves the base and ordered commits and writes them to the run summary; only a second run with `confirm` enabled mutates refs. Blank cherry-picks default to commits from merged `stable-candidate` PRs that are in `main` after the base, in `main` order. An explicit list replaces the default. The base must be a nightly tag in `main` unless `allow_untagged_base` is deliberately enabled. Cherry-pick conflicts and release-prep reconciliation conflicts halt without resolution or remote mutation.

During 0.x, an auto-derived stable version increments minor for either a breaking Conventional Commit or `feat`, and patch otherwise. An exact advancing `0.x.y` override is accepted. Release prep cuts the Unreleased changelog, updates the workspace version and internal exact dependency versions, refreshes `Cargo.lock`, and passes the full whole-tree gate. The same release-prep commit must cherry-pick onto current `staging`; the workflow atomically pushes that reconciliation branch with `stable` and the stable tag, then opens a small PR into `staging`.

cargo-dist remains the stable artifact publisher. `dispatch-releases = true` lets promotion explicitly dispatch the generated `release.yml` after creating the stable tag over SSH with the release deploy key; keeping the generated workflow dispatch-only prevents deploy-key-authenticated nightly tag pushes from invoking cargo-dist. The generated release API therefore receives an existing stable tag, while the nightly release command uses `--verify-tag`; neither release API may create a tag implicitly. `publish-prereleases = false` keeps Homebrew stable-only. The generated workflow is regenerated, never hand-edited.

## Rulesets

GitHub rejects the GitHub Actions `Integration` bypass actor on the user-owned `mdml/sub` repository with HTTP 422 because the integration is not part of the ruleset source or an owner organization. The current [create-ruleset API](https://docs.github.com/en/rest/repos/rules?apiVersion=2026-03-10#create-a-repository-ruleset) defines `DeployKey` as a bypass actor, says its `actor_id` should be null, and excludes `pull_request` mode; omitting the null ID gives the exact request shape `{ "actor_type": "DeployKey", "bypass_mode": "always" }`.

The trust boundary is: only holders of the release deploy key can bypass the stable and release-tag rulesets, and the private key lives solely in this repository's Actions secrets. Any workflow in the repository can reference a repository secret, so a changed or new workflow could use the key; this is the same repository-workflow trust boundary as the rejected GitHub Actions integration bypass. The nightly publish job and confirmed stable-promotion job check out with `RELEASE_DEPLOY_KEY`, persist its SSH credentials for Git, and refuse ref writes unless `origin`'s push URL is SSH. Their API calls continue to use `GITHUB_TOKEN`, which cannot bypass either ruleset.

The owner must generate one keypair, add its public half as a write-enabled repository deploy key, store its private half as `RELEASE_DEPLOY_KEY`, verify both entries, and remove the local keypair:

```sh
release_key_dir=$(mktemp -d)
ssh-keygen -t ed25519 -C "mdml/sub release channels" -f "$release_key_dir/id_ed25519" -N ""
gh api --method POST -H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2026-03-10" repos/mdml/sub/keys -f title="release channels" -f key="$(cat "$release_key_dir/id_ed25519.pub")" -F read_only=false
gh secret set RELEASE_DEPLOY_KEY --repo mdml/sub < "$release_key_dir/id_ed25519"
gh api -H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2026-03-10" repos/mdml/sub/keys --jq '.[] | select(.title == "release channels") | {id, title, read_only, verified, enabled}'
gh secret list --repo mdml/sub | grep '^RELEASE_DEPLOY_KEY'
rm "$release_key_dir/id_ed25519" "$release_key_dir/id_ed25519.pub"
rmdir "$release_key_dir"
unset release_key_dir
```

The owner must create the stable ruleset before the branch exists:

```sh
gh api --method POST -H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2026-03-10" repos/mdml/sub/rulesets --input - <<'JSON'
{
  "name": "stable",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [
    {"actor_type": "DeployKey", "bypass_mode": "always"}
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

The owner must also create a tag ruleset so only the deploy-key holder can create, move, or delete release tags:

```sh
gh api --method POST -H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2026-03-10" repos/mdml/sub/rulesets --input - <<'JSON'
{
  "name": "release tags",
  "target": "tag",
  "enforcement": "active",
  "bypass_actors": [
    {"actor_type": "DeployKey", "bypass_mode": "always"}
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

The design keeps `main` equal to the dogfooded source that was verified, makes stable composition explicit and reviewable, and prevents release preparation from becoming an untracked divergence. The deploy key supplies the repository-local identity that GitHub's user-owned-repository rulesets accept without giving a human bypass. The preview/confirm split is the smallest auditable confirmation mechanism available to `workflow_dispatch`. Whole-tree CodeScene in nightly and stable gates is the standing backstop for health drift in unchanged files.

## Revisit when

Revisit if cargo-dist accepts a prerelease release tag without requiring a matching prerelease workspace version, GitHub rulesets gain workflow-file-scoped deploy-key controls, repository-secret access becomes narrower than the workflow trust boundary, candidate discovery approaches the 100-PR query limit, or stable composition needs more than conflict-free cherry-picks.
