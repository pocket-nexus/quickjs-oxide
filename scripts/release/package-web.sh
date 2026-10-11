#!/usr/bin/env bash
set -euo pipefail

# Archives the playground site and the wasm-bindgen packages from the build
# that scripts/web/build-web-playground.sh just produced.
[[ $# -eq 2 ]] || { echo 'usage: package-web.sh <version> <out-dir>' >&2; exit 2; }
version="$1"
out_dir="$2"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
wasm_bindgen_version="0.2.126"
wasm_stem="quickjs_oxide_web"
pages_dir="${repo_root}/target/pages"
cargo_target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"
if [[ "${cargo_target_dir}" != /* ]]; then
  cargo_target_dir="${repo_root}/${cargo_target_dir}"
fi
wasm_file="${cargo_target_dir}/wasm32-unknown-unknown/web/${wasm_stem}.wasm"

[[ -f "${pages_dir}/index.html" && -f "${wasm_file}" ]] || {
  echo 'missing playground build; run scripts/web/build-web-playground.sh first' >&2
  exit 1
}
actual_wasm_bindgen_version="$(wasm-bindgen --version | awk '{print $2}')"
if [[ "${actual_wasm_bindgen_version}" != "${wasm_bindgen_version}" ]]; then
  echo "wasm-bindgen CLI ${actual_wasm_bindgen_version} does not match crate ${wasm_bindgen_version}" >&2
  exit 1
fi

mkdir -p "${out_dir}"
out_dir="$(cd "${out_dir}" && pwd)"
staging="$(mktemp -d)"
trap 'rm -rf "${staging}"' EXIT

playground="quickjs-oxide-playground-v${version}"
tar -czf "${out_dir}/${playground}.tar.gz" -C "${pages_dir}" .
echo "${out_dir}/${playground}.tar.gz"

wasm="quickjs-oxide-wasm-v${version}"
mkdir "${staging}/${wasm}"
for bindgen_target in web nodejs; do
  wasm-bindgen \
    "${wasm_file}" \
    --out-dir "${staging}/${wasm}/${bindgen_target}" \
    --out-name "${wasm_stem}" \
    --target "${bindgen_target}"
done
cp "${repo_root}/LICENSE" "${repo_root}/NOTICE" "${staging}/${wasm}/"
cp -R "${repo_root}/LICENSES" "${staging}/${wasm}/"
tar -czf "${out_dir}/${wasm}.tar.gz" -C "${staging}" "${wasm}"
echo "${out_dir}/${wasm}.tar.gz"
