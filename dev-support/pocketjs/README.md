# PocketJS Hero code-image trial

Apply [the PocketJS patch](oxide-script-image.patch) to a writable checkout of
the PocketJS Oxide trial at `17f5b817c40c787aa7aaa6c73037d2e7a5b96b69`. The
patch extends the existing package section 10 and uses the same guest
dispatch path for both image-only and source-capable Oxide hosts. It keeps
the container version and the original app plan and asset pack. The
`--oxide-only` packer accepts UI-only apps such as stock Hero.

The current encoder produces OXBC v4. Build the exporter and guest from
the same core checkout so their fingerprints match. The guest reuses the
native asset registry and creates the private `__pak` buffer on first
access for packages with assets already mounted. The core can execute
directly from owned or static package storage. A host that retains an
admitted `ScriptImage` can supply it via `OxideGuest::with_verified_image`.
The default file path verifies the image at boot. The initial accessor
becomes a writable data property after the first read or assignment to
`globalThis.__pak`.

Set `OXIDE` to this checkout, `POCKETJS` to the PocketJS checkout and
`OUT` to a new writable directory. Set `HERO` to the original stock Hero
`.pocket` and `POCKETJS_PROFILE_RS` to the matching generated host profile.
The acceptance run used `product-linux-kms`, 480×272, at 60 Hz.

```sh
git -C "$POCKETJS" apply "$OXIDE/dev-support/pocketjs/oxide-script-image.patch"
cargo build --manifest-path "$OXIDE/Cargo.toml" --release --example compile-image

cd "$POCKETJS"
mkdir -p "$OUT" .cargo
cat > .cargo/oxide-trial.toml <<CONFIG
[patch.crates-io]
quickjs-oxide = { path = "$OXIDE" }
quickjs-oxide-core = { path = "$OXIDE/core" }
quickjs-oxide-regexp = { path = "$OXIDE/regexp" }
CONFIG
bun tools/packaging/compile-oxide.ts "$OXIDE/target/release/examples/compile-image" \
  "$HERO" product-linux-kms "$OUT/hero-oxide.pocket" --oxide-only
bun tools/pocket-pack.ts verify "$OUT/hero-oxide.pocket"
cargo +1.99.0 --config .cargo/oxide-trial.toml build --release -p pocket-linux-kms \
  --no-default-features --features oxide-image --example guest_compare
cp target/release/examples/guest_compare "$OUT/core-only"
POCKET_OP_TRACE="$OUT/oxide.trace" "$OUT/core-only" --hero-press oxide \
  "$OUT/hero-oxide.pocket" 360 "$OUT/oxide.ppm"
```

`oxide-image` links the Oxide core alone. `oxide-guest` additionally links
the source frontend and enables source evaluation after the guest declines
an incompatible image. The C comparison feature uses the existing
PocketJS QuickJS guest and its `QJBC` loader. The diagnostic packer calls
the existing PocketJS QuickJS compiler and uses an ABI digest for the
native comparison build. Its digest differs from the ESP-IDF deployment ABI.

Set `POCKETJS_QUICKJS_SOURCE` to a local QuickJS-NG 0.16.2 source tree and
use bytecode version 28 for the native comparison.

```sh
export POCKETJS_QUICKJS_ABI="$OUT/quickjs-abi.bin"
bun tools/packaging/compare-qjbc.ts "$HERO" product-linux-kms \
  "$POCKETJS_QUICKJS_SOURCE" 0.16.2 28 "$OUT/hero-qjbc.pocket" "$POCKETJS_QUICKJS_ABI"
cargo +1.99.0 --config .cargo/oxide-trial.toml build --release -p pocket-linux-kms \
  --no-default-features --features oxide-image,qjbc-compare --example guest_compare
cp target/release/examples/guest_compare "$OUT/compare"
POCKET_OP_TRACE="$OUT/qjbc.trace" "$OUT/compare" --hero-press qjbc \
  "$OUT/hero-qjbc.pocket" 360 "$OUT/qjbc.ppm"
cmp "$OUT/oxide.trace" "$OUT/qjbc.trace"
cmp "$OUT/oxide.ppm" "$OUT/qjbc.ppm"
```

The trace includes boot, a delimiter for each replay frame and ordered host
operation IDs, arguments and results. The replay uses identical button edges
for both guests. Use the same comparison binary and `--boot-only` with zero
frames and tracing disabled for native timing. This measures package admission,
asset mounting and guest evaluation through `boot_deferred`. The no-draw
native comparison is diagnostic. A device rollout still needs the release
QuickJS QJBC ABI and an on-device measurement.
