#!/usr/bin/env bash
set -euo pipefail

# Sets the version every crate shares: [workspace.package] and the members'
# entries in [workspace.dependencies], then the members in Cargo.lock.
[[ $# -eq 1 ]] || { echo 'usage: set-workspace-version.sh <version>' >&2; exit 2; }
version="$1"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

# [workspace.package] holds the only line-leading `version =` key; the
# members' entries in [workspace.dependencies] carry it as well.
python3 - "${version}" <<'PY'
import pathlib, re, sys
version = sys.argv[1]
manifest = pathlib.Path("Cargo.toml")
text, count = re.subn(r'(?m)^version = "[^"]*"$', f'version = "{version}"',
                      manifest.read_text(), count=1)
text, members = re.subn(r'(?m)^([\w-]+ = \{ path = "[^"]*", version = ")[^"]*"',
                        rf'\g<1>{version}"', text)
if count != 1 or members != 5:
    sys.exit(f"expected 1 package version and 5 member versions, found {count} and {members}")
manifest.write_text(text)
PY
[[ "$(./scripts/release/workspace-version.sh)" == "${version}" ]] || {
  echo 'failed to update the workspace version in Cargo.toml' >&2
  exit 1
}
cargo update --workspace --quiet
