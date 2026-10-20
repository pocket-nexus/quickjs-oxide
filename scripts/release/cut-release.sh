#!/usr/bin/env bash
set -euo pipefail

# Sets the workspace version, regenerates apps/cli/qjs.order, commits both
# and creates the matching `v` tag.
# Pushing the tag starts .github/workflows/release.yml.
[[ $# -eq 1 ]] || { echo 'usage: cut-release.sh <version>  (e.g. 0.1.0)' >&2; exit 2; }
version="$1"
if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
  echo "not a release version: ${version}" >&2
  exit 2
fi
tag="v${version}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo 'tracked files have uncommitted changes' >&2
  exit 1
fi
if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
  echo "tag ${tag} already exists" >&2
  exit 1
fi
# Before anything changes: regenerating apps/cli/qjs.order needs valgrind
# and the fixed-work bundles.
python3 scripts/release/gen-symbol-order.py --check-inputs

current="$(./scripts/release/workspace-version.sh)"
if [[ "${current}" != "${version}" ]]; then
  # [workspace.package] holds the only line-leading `version =` key.
  python3 - "${version}" <<'PY'
import pathlib, re, sys
manifest = pathlib.Path("Cargo.toml")
text, count = re.subn(r'(?m)^version = "[^"]*"$', f'version = "{sys.argv[1]}"',
                      manifest.read_text(), count=1)
manifest.write_text(text)
PY
  [[ "$(./scripts/release/workspace-version.sh)" == "${version}" ]] || {
    echo 'failed to update the workspace version in Cargo.toml' >&2
    exit 1
  }
  cargo update --workspace --quiet
  # The order file names mangled symbols, whose crate hashes change with the
  # version: regenerate it for the release build.
  cargo build --locked --release --quiet -p quickjs-oxide-cli --bin qjs
  python3 scripts/release/gen-symbol-order.py
  git commit --quiet -m "release: ${tag}" -- Cargo.toml Cargo.lock apps/cli/qjs.order
fi
git tag -a "${tag}" -m "quickjs-oxide ${tag}"

branch="$(git branch --show-current)"
echo "Tagged ${tag} at $(git rev-parse --short HEAD). Publish with:"
echo "  git push --atomic origin ${branch:-HEAD} ${tag}"
