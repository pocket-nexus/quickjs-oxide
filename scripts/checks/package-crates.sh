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

# Cargo reuses a package's unpacked sources whenever their directory
# exists, and the pre-release version repeats from commit to commit; a
# copy unpacked from another commit (a restored Cargo cache) would build
# in place of the packaged one.
rm -rf "${CARGO_HOME:-${HOME}/.cargo}"/registry/src/*/quickjs-oxide*-"${unpublished}"

./scripts/release/set-workspace-version.sh "${unpublished}"
cargo package --locked --allow-dirty \
  -p quickjs-oxide-regexp -p quickjs-oxide-core -p quickjs-oxide \
  -p quickjs-oxide-host -p quickjs-oxide-cli
