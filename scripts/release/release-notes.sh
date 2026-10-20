#!/usr/bin/env bash
set -euo pipefail

# Prints the install section that heads each GitHub Release body; GitHub's
# generated change list follows it.
[[ $# -eq 2 ]] || { echo 'usage: release-notes.sh <version> <owner/repo>' >&2; exit 2; }
version="$1"
repository="$2"

cat <<EOF
## Install \`qjs\`

Prebuilt binaries for Linux (static musl), macOS and Windows are attached below.

\`\`\`sh
# Prebuilt binary through cargo-binstall
cargo binstall --git https://github.com/${repository} quickjs-oxide-cli
# Build from source (Rust 1.99 or newer)
cargo install --locked --git https://github.com/${repository} --tag v${version} quickjs-oxide-cli
\`\`\`

## WebAssembly

- \`quickjs-oxide-wasm-v${version}.tar.gz\`: wasm-bindgen packages for browsers (\`web/\`, ES module) and Node.js (\`nodejs/\`).

\`SHA256SUMS\` lists the digest of every attached file.
EOF
