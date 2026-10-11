#!/usr/bin/env bash
set -euo pipefail

# Prints [workspace.package].version, the single version every crate shares.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
python3 - "${repo_root}/Cargo.toml" <<'PY'
import sys, tomllib
with open(sys.argv[1], "rb") as manifest:
    print(tomllib.load(manifest)["workspace"]["package"]["version"])
PY
