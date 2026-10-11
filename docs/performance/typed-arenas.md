# Typed heap storage

The heap stores Object, Context and FunctionBytecode in a shared arena, captured-variable cells and Shape in separate typed arenas, and String and BigInt in a shared leaf arena. IDs such as `VarRefId` and `ShapeId` carry a full generational identity. The main implementation is in `src/engine/heap/arena.rs`, `auxiliary_arena.rs`, `identity.rs`, `binding_storage.rs`, `object_storage.rs`, `gc.rs` and `src/engine/object/shape.rs`.

## Captured variables and shapes

A captured-variable cell is a mutable binding shared by active frames and closures. Ordinary replacement takes an owner for the new edge and releases the replaced edge; owned replacement transfers the incoming owner and hands back the replaced owner. Shape storage maintains copy-on-write for shared layouts, in-place updates for uniquely owned layouts, layout revisions, dictionary order, prototype edges, property-key atom edges and weak interner cleanup.

Each typed arena manages slots, free indexes, generations, reference counts and states. Allocation and publication establish the record identity; release enters the zero-count queue; collection follows reference edges across arenas by full `RawId`. Cycle collection uses each arena's trial/reachability space and keeps the finalization anchors of Object, Context and FunctionBytecode.

## Capacity and diagnostics

After an explicit `Runtime::run_gc` completes and clears deferred roots, an empty zero-count queue keeps at most 4,096 ID slots. Ordinary reference release reuses the queue backing. Profiling builds report per-arena slot capacity, free count, queue and collection scratch; the [diagnostics reference](../profiling.md) defines the accounting boundaries of these fields.

Ownership and borrows pass between the VM, the heap and held state according to the [execution invariants](principles.md). Tests are in `src/engine/heap/tests.rs`, `src/engine/heap/runtime/tests/gc.rs` and the heap storage/collection test modules.
