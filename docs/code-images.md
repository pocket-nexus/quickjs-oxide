# Oxide code images

Build a prebundled classic script into an Oxide image on the development host.

```sh
cargo build --release --example compile-image
target/release/examples/compile-image --strip-source app.js app.oxbc
```

The input is the exact bundle bytes. Remove the container's NUL terminator when
extracting the bundle from a PocketJS package. The exporter uses the regular
script compiler and register lowering and does not execute the bundle. It
keeps function source and positions by default. `--strip-source` keeps the
positions and omits the function text used by `Function.prototype.toString`.

An embedder can depend on `quickjs-oxide-core` and use the same image on a
32-bit or 64-bit target. Mount the destination's host bindings on its own
runtime. Call `runtime.load_image(bytes)`, pass the returned function directly
to `runtime.run_script`, and drain jobs at the ordinary host boundary. The
function is unrooted until the caller runs or holds it. The source compiler is
an optional host hook. Without it, runtime operations that require compiling
new source report the core's existing compilation error.

`LoadError::Rejected` is the format fallback boundary. It occurs before
the loader publishes code into the destination runtime. A host that has a
source compiler may try the source after this error. An allocation failure
from a compatible image, a guest exception and a failed job are boot
failures. Tear down the failed application instance in these cases.

A package installer can call `ScriptImage::decode_shared` once, then
hand the verified value to the runtime. A clone of the proof shares the
immutable instruction and position data and can be mounted in another realm.
Use `ImageBytes::new` for a range of an owned package, or `ImageBytes::from`
with a static slice. The raw `load_image` convenience API copies the input
and checks it. The verification result is an in-process object.

## OXBC v4

All integers are little-endian. The header is 24 bytes.

| Field | Width in bytes |
| --- | ---: |
| `OXBC` magic | 4 |
| Version, currently 4 | 2 |
| Header size | 2 |
| Payload length | 4 |
| Engine fingerprint | 8 |
| CRC-32 of the payload | 4 |

The same core source and version produce the same fingerprint regardless of
build target or profile. Rebuild the image with the core used by the
destination host. PocketJS uses package section 10 for both this format and
earlier experimental formats. The compiler and loader here use v4.

The payload has a common filename, a Latin-1 or UTF-16 string table and the
script's function units in postorder. Common instructions occupy four
bytes. The first byte identifies the opcode and the next three hold either
the operand bytes, or packed operands and an immediate. Each unit specifies
the width of its packed operand fields. If the immediate does not fit, the
instruction names a record in its unit's wide side table. Both forms use
instruction indexes for control flow. The host selects an execution policy when constructing the runtime. The same
verified image can be instantiated under any policy.

```rust
use oxide_core::{ImageDecodePolicy, Runtime, RuntimeOptions};

let policy = ImageDecodePolicy::Adaptive { max_bytes: 16 * 1024 };
let mut runtime = Runtime::new_with_image_policy(RuntimeOptions::default(), policy)?;
let script = runtime.load_image(&image_bytes)?;
runtime.run_script(script)?;
```

* `Packed` reads instructions from the image for the lifetime of the runtime.
* `Adaptive { max_bytes }` shares a byte budget across all images in the
  runtime. The default is 32 KiB. A second frame entry or a hot loop backedge
  poll can decode a function. Its instructions stay resident for the runtime's
  lifetime. A function larger than the remaining budget continues to execute
  packed, including when an earlier phase has used up that budget. The limit
  covers decoded instruction arrays.
* `Eager` allocates and decodes every function during installation. Insufficient
  memory for these arrays fails installation before the functions are published.

Hosts with custom services can use `Runtime::with_host_and_image_policy`.
Decoded functions share the source interpreter's dispatch and resolve property
operands in their own realm.

Source positions use a one-byte encoding for small changes on the same line.
Other changes store a short PC delta in the record header, with an extended
delta for longer jumps. Line deltas are present only when the line changes.
Line and column deltas use ZigZag encoding. The
interpreter reads this table when it needs a source location. Property-key
operands refer to the image's string table. The runtime maps them to its own
atoms through a per-image table. BigInts carry a sign and limbs. Regular
expression literals carry their pattern and flags. Tagged templates carry
cooked and raw strings and are created with the destination realm's array
prototype. Units eligible for short-call plans reserve their inline caches on publication.
Other units allocate their caches on first execution.

Admission checks the header, fingerprint, CRC, section bounds, function tree
and core publication invariants. The validator reads instructions directly
and takes no temporary instruction or position array. Instantiation publishes
the admitted units with consecutive static IDs and builds realm-specific constant tables
and bytecode records. Use a trusted image producer and authenticate
distribution separately. The payload carries code and literals and the
destination builds its own realm state. Static module linking and PocketJS
worker boot are outside this classic-script path.

The PocketJS integration and the Hero acceptance commands are documented
[in the PocketJS trial](../dev-support/pocketjs/README.md).
