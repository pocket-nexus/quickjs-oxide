# B2: own field promoted into a local

Implementation: `85aa1956`, stacked on A2. A direct receiver field read
followed by `SetLocal` and `Drop` now has a guarded entry before its ordinary
selected fallback. A successful own-data cache peek retains the current field
value, moves that new owner into a pre-admitted direct local, publishes the
write position, and releases the displaced owner through the existing frame
binding protocol. It never reports a replayable miss after promotion or local
replacement. Getters, proxies, missing fields, cold caches, uncertain
bindings, and unsupported cleanup states execute the ordinary path once.
The guard and fallback share the logical property cache site.

## Local mechanism and fixed work

The Node-authenticated development workload invokes an admitted
`node=node.next` function 200,000 times. Profiling recorded 200,000
`field_local_assign` attempts, 199,999 hits, and one cache-warming miss.
For the same script, the A2 and B2 profiling builds recorded respectively
600,000 versus 400,001 `slot_copy.ObjectRetain` events and 1,400,017 versus
1,200,018 `runtime_pc_publication` events. These are scoped mechanism
counters, not a complete retain/release or instruction accounting. Both
profile records are in [data](data).

Four balanced ABBA/BAAB fresh-process samples per engine and workload used
plain release binaries. The ratios below are medians of whole-process wall
time, including startup and compilation; the adjacent A/A uses B2 on both
sides. Exact outputs, process samples, hashes and build receipts are in
[data](data).

| Workload | B2 / A2 wall | Same-binary A/A wall |
| --- | ---: | ---: |
| Own object-valued field assignment | 0.969× | 0.989× |
| Accessor field assignment | 1.029× | 0.998× |

The own-data gain is consistent with the removed owner copy and continuation.
The always-accessor loss is a material cost of a failed guard and remains
visible. These synthetic cases are development diagnostics, not independent
application holdouts. A pinned V8-v7 Richards profile found no selected B2
site; application transfer is evaluated separately in V1.

The standalone Script compile probe ran a 500-function selected assignment
corpus and the lexical-loop control in 24 alternating fresh-process samples
per engine. B2/A2 median compile-time ratios were 0.988× and 0.954×; the
same-binary A/A ratios were 0.956× and 0.958×. The apparent gains are within
drift. The probe excludes runtime/context construction, source I/O, and
teardown.

The B2 plain binary is 9,111,056 bytes, 12,328 bytes larger than A2. Its
SHA-256 is `45ccc5046c2a446e575a00ba7c9c8646b4ba66b65c3d9bf1cededa18cef8bff1`.
Raw archive SHA-256: A/B
`fa20d51c4a2d77424791ab6970dd6686b90342d65faf29a1c3f1e10ae7db7567`;
A/A `245bed487d79acd40426f5ebac2018a33f49569b4f5bd26ee64ffa07fc8b128f`;
compile A/B `a8fc299f0a29e5ab4144624f7e20f58ac6c43a10c9a1466f674c6653269d06f5`;
compile A/A `c2fc8578824026fa7ea3faa6a0445514f1959c3c7398034420c401c9cdf0e5b7`.

Focused differential tests cover self references, an object-valued chain,
scalar and owner-bearing displaced locals, current-value reads after a
same-shape value change, data-to-accessor change, getter/proxy execution and
thrown identity, missing fields, captured and lexical bindings, and wide
field-site positions. The full workspace suite passed 2,017 engine tests and
908 oracle tests (one ignored), plus other targets and 42 benchmark-tool
tests. Rust 1.88 workspace Clippy passed with warnings denied.
