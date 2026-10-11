# Heap storage and collection

heap owns runtime records, generational identity, reference edges, roots,
reference counts and cycle collection. It stores the raw payloads of objects,
code, realms, modules, Promises and suspended state; the semantic modules own
the JavaScript algorithms over those payloads.

Records describe what is stored; storage operations define how records and
their reference edges change. Allocation, publication, replacement and
release keep ownership consistent so that no edge is retained twice or
released early. Deferred cleanup and cycle collection use the same record
model to handle weak references and cascading destruction.

This module also manages the Runtime's shared heap and Atom state. High-level
property access, numeric conversion, module evaluation and builtin algorithms
use storage through their own Runtime methods and stay outside the heap
implementation. Storage changes distinguish physical slot position, stable
identity and observable order.

The VM uses reference counting and cycle collection. See
[workspace architecture](../../../docs/architecture.md) for the hand-off
between active owning values and long-lived suspended raw reference edges.

Objects, Contexts and FunctionBytecode live in the shared arena; captured
variable cells and Shapes each have their own generational arena, and
String/BigInt use the leaf arena. Numeric indexes can coincide across arenas,
so cross-arena references and collection use the full typed identity. Cell
and shape slots carry no object-specific weak-link state; cycle collection
traces their edges, and object, Context and FunctionBytecode anchors trigger
cleanup of unreachable cycles. See
[typed arenas](../../../docs/performance/typed-arenas.md) for design and
measurements. After a successful explicit GC and deferred root release, an
empty zero queue retains at most 4,096 ID slots; ordinary reference release
keeps the queue capacity for reuse. Tests in
`src/engine/heap/runtime/tests/gc.rs` cover capacity and collection behavior.
