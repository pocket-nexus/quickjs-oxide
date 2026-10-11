#!/usr/bin/env bash
set -euo pipefail

# Prints the install section that heads each GitHub Release body; GitHub's
# generated change list follows it.
[[ $# -eq 2 ]] || { echo 'usage: release-notes.sh <version> <owner/repo>' >&2; exit 2; }
version="$1"
repository="$2"
owner="${repository%%/*}"
name="${repository#*/}"

cat <<EOF
## Install \`qjs\`

Prebuilt binaries for Linux (static musl), macOS and Windows are attached below.

\`\`\`sh
# Prebuilt binary through cargo-binstall
cargo binstall --git https://github.com/${repository} quickjs-oxide-cli
# Build from source (Rust 1.95 or newer)
cargo install --locked --git https://github.com/${repository} --tag v${version} quickjs-oxide-cli
\`\`\`

## WebAssembly

- \`quickjs-oxide-wasm-v${version}.tar.gz\`: wasm-bindgen packages for browsers (\`web/\`, ES module) and Node.js (\`nodejs/\`).
- \`quickjs-oxide-playground-v${version}.tar.gz\`: the static playground site for this release, also served at <https://${owner}.github.io/${name}/releases/v${version}/>.

\`SHA256SUMS\` lists the digest of every attached file.
EOF
