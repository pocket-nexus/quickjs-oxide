#!/usr/bin/env bash
set -euo pipefail

# Packages the five published crates and builds each from its packaged
# sources alone. Cargo resolves a member's dependency on another member to
# the copy packaged beside it only when crates.io has no release of that
# version, so the packages are built at an unpublished pre-release of the
# next patch version; Cargo.toml and Cargo.lock are restored afterwards.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

current="$(./scripts/release/workspace-version.sh)"
unpublished="$(python3 - "${current}" <<'PY'
import re, sys
major, minor, patch = re.match(r"(\d+)\.(\d+)\.(\d+)", sys.argv[1]).groups()
print(f"{major}.{minor}.{int(patch) + 1}-ci")
PY
)"

backup="$(mktemp -d)"
cp Cargo.toml Cargo.lock "${backup}/"
trap 'cp "${backup}/Cargo.toml" "${backup}/Cargo.lock" . && rm -rf "${backup}"' EXIT

# The unpublished version repeats across commits. A restored Cargo cache can
# otherwise supply unpacked sources from an earlier commit instead of the
# archives packaged below. Invalidate only these five pre-release source trees.
for crate in quickjs-oxide-regexp quickjs-oxide-core quickjs-oxide \
             quickjs-oxide-host quickjs-oxide-cli; do
  rm -rf -- "${CARGO_HOME:-${HOME}/.cargo}"/registry/src/*/"${crate}-${unpublished}"
done

./scripts/release/set-workspace-version.sh "${unpublished}"
cargo package --locked --allow-dirty \
  -p quickjs-oxide-regexp -p quickjs-oxide-core -p quickjs-oxide \
  -p quickjs-oxide-host -p quickjs-oxide-cli
