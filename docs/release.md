# Releases

Every crate shares the `[workspace.package]` version. A `vX.Y.Z` tag on that
version publishes a GitHub Release, which is the only distribution channel;
every crate sets `publish = false`.

## Cutting a release

```sh
./scripts/release/cut-release.sh 0.1.0
git push --atomic origin main v0.1.0
```

`cut-release.sh` sets the workspace version, refreshes `Cargo.lock`, commits
`release: v0.1.0` and creates the annotated tag. The release workflow rejects a
tag that does not match the workspace version, so `cargo install --tag` and the
prebuilt binaries report the same version.

## What `.github/workflows/release.yml` publishes

| Asset | Contents |
| --- | --- |
| `qjs-vX.Y.Z-<target>.tar.gz` / `.zip` | `qjs` plus README and licenses for `x86_64`/`aarch64-unknown-linux-musl` (static), `x86_64`/`aarch64-apple-darwin` and `x86_64-pc-windows-msvc` |
| `quickjs-oxide-wasm-vX.Y.Z.tar.gz` | wasm-bindgen packages of `apps/web`: `web/` (ES module) and `nodejs/` |
| `quickjs-oxide-playground-vX.Y.Z.tar.gz` | The static playground site built from the tag |
| `SHA256SUMS` | Digests of every asset |

Native binaries use the pinned Rust toolchain; the WebAssembly build follows
the Pages playground's `scripts/web/build-web-playground.sh`. Each native
binary except the cross-compiled `x86_64-apple-darwin` one evaluates a smoke
expression before packaging. Re-running the workflow for an existing release
replaces its assets.

After publishing, the workflow dispatches `pages.yml` on the default branch.
The Pages build unpacks every release's playground archive under
`/releases/<tag>/` next to the main-branch playground and lists them at
`/releases/`.

## Installing

```sh
# Prebuilt binary (archive names match apps/cli's cargo-binstall metadata)
cargo binstall --git https://github.com/pocket-nexus/quickjs-oxide quickjs-oxide-cli
# From source at a release tag
cargo install --locked --git https://github.com/pocket-nexus/quickjs-oxide \
  --tag v0.1.0 quickjs-oxide-cli
```
