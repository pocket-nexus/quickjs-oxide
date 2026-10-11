#!/usr/bin/env bash
set -euo pipefail

# Unpacks every published release's playground archive under
# <pages-dir>/releases/<tag>/ and writes an index linking them. Needs `gh`
# with read access to the repository's releases (GH_TOKEN, GH_REPO in CI).
[[ $# -eq 1 ]] || { echo 'usage: assemble-release-playgrounds.sh <pages-dir>' >&2; exit 2; }
pages_dir="$1"
releases_dir="${pages_dir}/releases"
downloads="$(mktemp -d)"
trap 'rm -rf "${downloads}"' EXIT

rm -rf "${releases_dir}"
mkdir -p "${releases_dir}"

tags=()
while IFS= read -r tag; do
  [[ -n "${tag}" ]] || continue
  if [[ ! "${tag}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
    echo "skipping release with a non-version tag: ${tag}" >&2
    continue
  fi
  asset="quickjs-oxide-playground-${tag}.tar.gz"
  if ! gh release download "${tag}" --pattern "${asset}" --dir "${downloads}"; then
    echo "skipping ${tag}: no ${asset}" >&2
    continue
  fi
  mkdir "${releases_dir}/${tag}"
  tar -xzf "${downloads}/${asset}" -C "${releases_dir}/${tag}"
  tags+=("${tag}")
done < <(gh release list --limit 1000 --exclude-drafts \
  --json tagName,createdAt \
  --jq 'map(select(.tagName | startswith("v"))) | sort_by(.createdAt) | reverse | .[].tagName')

{
  cat <<'HTML'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>quickjs-oxide playground releases</title>
  </head>
  <body>
    <h1>quickjs-oxide playground releases</h1>
    <p><a href="../">Latest main build</a></p>
    <ul>
HTML
  for tag in ${tags[@]+"${tags[@]}"}; do
    printf '      <li><a href="./%s/">%s</a></li>\n' "${tag}" "${tag}"
  done
  cat <<'HTML'
    </ul>
  </body>
</html>
HTML
} > "${releases_dir}/index.html"

echo "Assembled ${#tags[@]} release playground(s) under ${releases_dir}"
