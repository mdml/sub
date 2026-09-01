# Bridge pinning and installation

Date: 2026-08-27. Status: adopted and implemented 2026-08-31.

## Decision

Each adapter crate declares, as constants in its source, the bridge package name, the exact bridge version, and the harness version range it was verified against. `sub` installs bridges once, through `sub onboard <harness>` or the lower-level `sub bridge install <harness>` action, into a `sub`-owned directory under the state directory, one subdirectory per bridge and version. Onboarding verifies an intact matching installation and reports it unchanged instead of invoking npm again; a missing, stale, or damaged installation is repaired through the existing installer. Installation of an npm-distributed bridge runs `npm install --prefix <dir> <package>@<exact version>` with `--ignore-scripts` where the bridge allows it, then records a manifest (package, version, install time, integrity hash of the installed tree). At launch, the adapter resolves the bridge binary from that manifest; if the manifest is missing or its version differs from the adapter's constant, launch fails with an error that names the install action. No adapter ever runs `npx`, resolves a `latest` tag, or downloads anything at launch. `cursor-agent` needs no bridge; its adapter records only the harness version. Bridge upgrades are ordinary pull requests that bump the constant, re-run the contract suite against the real harness, and update the adapter's declared versions.

## Rationale

The mental model states that bridges are pinned to exact versions and installed once by `sub`, never fetched at launch, and that adapters declare the harness and bridge versions they were verified against. Keeping the pin in the adapter's source ties the version to the code that was tested against it and makes the pair reviewable in one diff. A per-version install directory makes upgrades atomic and rollbacks trivial, and an integrity hash lets the supervisor detect a tampered or half-installed bridge before an attempt starts. Using the harness's own package manager (`npm`) rather than vendoring the bridges avoids redistributing third-party code under `sub`'s release signature, at the cost of requiring `npm` at install time; the spike found both bridges are npm packages (`@agentclientprotocol/codex-acp`, `@agentclientprotocol/claude-agent-acp`). Vendoring stays open as the fallback if the mental model's stated risk, "third-party ACP bridges stay maintained", materializes.

## Revisit when

A bridge ships as a prebuilt binary (install by checksum-verified download instead), or the owner decides to vendor bridges.
