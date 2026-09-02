#!/usr/bin/env sh
# Update the workspace version and cut CHANGELOG.md for one stable release.
set -eu

[ "$#" -eq 2 ] || {
    echo "usage: prepare-stable.sh 0.MINOR.PATCH YYYY-MM-DD" >&2
    exit 2
}
version="$1"
date="$2"
case "$version" in
    0.[0-9]*.[0-9]*) ;;
    *) echo "prepare-stable: version must be an exact 0.x.y version" >&2; exit 2 ;;
esac
case "$date" in
    [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]) ;;
    *) echo "prepare-stable: date must use YYYY-MM-DD" >&2; exit 2 ;;
esac

python3 - "$version" "$date" <<'PY'
import pathlib
import re
import sys

version, date = sys.argv[1:]
manifest_path = pathlib.Path("Cargo.toml")
manifest = manifest_path.read_text()
manifest, count = re.subn(
    r'(\[workspace\.package\]\nversion = ")[^"]+("\n)',
    rf'\g<1>{version}\2',
    manifest,
    count=1,
)
if count != 1:
    raise SystemExit("prepare-stable: workspace version field not found")
manifest = re.sub(
    r'(sub(?:-sdk|-harness-fake|-adapter-(?:claude|codex|cursor)) = \{ path = "[^"]+", version = ")[^"]+(" \})',
    rf'\g<1>={version}\2',
    manifest,
)
manifest_path.write_text(manifest)

changelog_path = pathlib.Path("CHANGELOG.md")
changelog = changelog_path.read_text()
marker = "## [Unreleased]\n"
if changelog.count(marker) != 1:
    raise SystemExit("prepare-stable: CHANGELOG.md must contain one Unreleased heading")
changelog_path.write_text(changelog.replace(marker, f"{marker}\n## [{version}] - {date}\n", 1))
PY

# Refresh only workspace-package entries in the existing lockfile. The locked
# full gate that follows proves no dependency resolution drift occurred.
cargo metadata --format-version 1 >/dev/null
