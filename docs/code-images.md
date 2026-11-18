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

## OXBC v3

All integers are little-endian. The header is 24 bytes.

| Field | Width in bytes |
| --- | ---: |
| `OXBC` magic | 4 |
| Version, currently 3 | 2 |
| Header size | 2 |
| Payload length | 4 |
| Engine fingerprint | 8 |
| CRC-32 of the payload | 4 |

The same core source and version produce the same fingerprint regardless of
build target or profile. Rebuild the image with the core used by the
destination host. PocketJS uses package section 10 for both this format and
earlier experimental formats. The compiler and loader here use v3.

The payload has a common filename, a Latin-1 or UTF-16 string table and the
script's function units in postorder. Each unit has typed instructions,
handlers, child references, captures, function metadata, debug information
and a literal pool. An instruction stores the opcode and its three byte
operands followed by a canonical unsigned base-128 immediate. Source
positions use an unsigned PC delta and ZigZag line and column deltas
relative to the previous position, starting at PC zero and the definition
location. String references, including instruction keys, are
resolved in the destination runtime. Immediate numeric property indexes keep
their encoding. BigInts carry a sign and limbs. Regular expression literals
carry their pattern and flags. Tagged templates carry cooked and raw strings
and are created with the destination realm's array prototype. Cache storage
is allocated on publication.

The loader checks the header, fingerprint, payload checksum, bounds, opcodes,
references, function-tree structure and the core publication validator
before instantiation. Publication then relocates the validated units and
passes their explicit tree links to the core bytecode allocator. The checksum detects corruption. Use a trusted image
producer and authenticate distribution separately. The payload carries code
and literals. It omits realm objects, inline cache contents, capabilities and
host state. Static module linking and PocketJS worker boot are outside this
classic-script path.

The PocketJS integration and the Hero acceptance commands are documented
[in the PocketJS trial](../dev-support/pocketjs/README.md).
