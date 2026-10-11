#!/usr/bin/env bash
set -euo pipefail

# Archives one release build of `qjs` under the name that
# apps/cli/Cargo.toml's cargo-binstall metadata resolves.
[[ $# -eq 3 ]] || {
  echo 'usage: package-cli.sh <version> <target-triple> <out-dir>' >&2
  exit 2
}
version="$1"
target="$2"
out_dir="$3"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cargo_target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"
if [[ "${cargo_target_dir}" != /* ]]; then
  cargo_target_dir="${repo_root}/${cargo_target_dir}"
fi

exe_suffix=""
[[ "${target}" != *-windows-* ]] || exe_suffix=".exe"
binary="${cargo_target_dir}/${target}/release/qjs${exe_suffix}"
[[ -f "${binary}" ]] || { echo "missing release binary: ${binary}" >&2; exit 1; }

stem="qjs-v${version}-${target}"
mkdir -p "${out_dir}"
out_dir="$(cd "${out_dir}" && pwd)"
staging="$(mktemp -d)"
trap 'rm -rf "${staging}"' EXIT

mkdir "${staging}/${stem}"
cp "${binary}" "${staging}/${stem}/"
cp "${repo_root}/README.md" "${repo_root}/LICENSE" "${repo_root}/NOTICE" \
  "${staging}/${stem}/"
cp -R "${repo_root}/LICENSES" "${staging}/${stem}/"

if [[ -n "${exe_suffix}" ]]; then
  (cd "${staging}" && 7z a -tzip -bso0 "${out_dir}/${stem}.zip" "${stem}")
  echo "${out_dir}/${stem}.zip"
else
  tar -czf "${out_dir}/${stem}.tar.gz" -C "${staging}" "${stem}"
  echo "${out_dir}/${stem}.tar.gz"
fi
