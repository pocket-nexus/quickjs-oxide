#!/usr/bin/env bash
set -euo pipefail

# Runs every Cargo example the docs pages include (`<!-- include:
# examples/NAME.rs... -->` in apps/web/docs/*.md). Each example asserts what
# its page shows, so a page cannot drift from the API it documents.
[[ $# -eq 0 ]] || { echo 'usage: test-doc-examples.sh' >&2; exit 2; }

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

mapfile -t examples < <(
  sed -n 's|^<!-- include: examples/\([A-Za-z0-9_-]*\)\.rs.* -->$|\1|p' apps/web/docs/*.md | sort -u
)
if [[ ${#examples[@]} -eq 0 ]]; then
  echo "no docs page includes an example" >&2
  exit 1
fi

cargo build --locked --quiet --examples
for example in "${examples[@]}"; do
  if ! output="$(cargo run --locked --quiet --example "${example}" 2>&1)"; then
    printf '%s\n' "${output}" >&2
    echo "docs example ${example} failed" >&2
    exit 1
  fi
  echo "docs example ${example}: ok"
done
