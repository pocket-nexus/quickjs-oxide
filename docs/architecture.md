# Workspace architecture

`quickjs-oxide` is the root engine package. Native and web adapters provide host capabilities; CLI, browser playground and Test262 runner use the engine through its public API. The current execution paths are described in [performance documentation](performance/README.md).

## Source owners

```text
src/
  source/          source bytes, positions, strings and Unicode data
  regexp/          pattern compilation and matching
  engine/
    compiler/      parsing, resolution, lowering and local operation selection
    code/          instruction encoding, verification and publication
    value/         JavaScript values, Number and conversion algorithms
    object/        properties, shapes and object internal methods
    atom/          names, symbols and property keys
    heap/          records, roots, storage, retention and collection
    vm/            frames, instruction execution, drivers and suspension
    realm/         global bindings, prototypes and realm initialization
    builtins/      builtin algorithms and native call dispatch
    modules/       module loading, linking and evaluation
    jobs/          queued computations and retained roots
    host/          environment capability contracts
    api/           public Runtime, Context and embedding operations
```

The root package has no production dependency on adapters or applications. Applications supply input and module-loading policy, output, diagnostics and job driving. `Runtime::new_with_host_services` receives the host capability provider.

## Compilation and publication

The compiler builds temporary `Instruction` values from parsed and resolved source. Lowering uses a temporary data-flow and effect graph to select bounded numeric operations. Publication validates stack states, control-flow targets, operand widths, instruction boundaries, resource limits and selected operation descriptors. It then encodes one `ExecCode` word stream and releases the temporary instruction graph.

Each execution word carries an opcode header and short operand; wide operands occupy following words. The published function keeps constants, linked identities, source positions, instruction boundaries and operation metadata. Runtime frames use execution-word positions for faults, fallthrough and resumption.

The numeric operation set includes local accumulation, product destination writes, array element copies and updates, preincrement-index addition, combined array/local writes and comparison branches. Selection recognizes finite instruction shapes. Runtime admission checks current binding values and genuine Array element storage; success consumes scoped frame or heap access. The ordinary instructions in the same stream provide the semantic continuation for other input states.

Publication also selects bounded fast entries for Number increments from locals or parameters, dense Array element reads and index updates, direct Number comparisons with branches, and field or array reads feeding a local numeric update. Their runtime guards check current values and output capacity before completing the published span. An admitted scalar `PutArrayEl` can update dense element storage within the execution loop.

## VM execution

`src/engine/vm/execute.rs` contains the instruction loop. `FrameCursor` carries the active frame transaction and fault/resume positions. `SlotStore` owns argument, parameter, local and operand windows; `FrameSlots` provides admitted short-lived access. A frame, heap owner or owning pending state roots each value across its lifetime.

Bytecode call inputs, function/capture owners and active-frame restoration records store owned heap IDs. Retirement releases their edges explicitly through the current `RuntimeState`; uninstalled entries stay under a borrowed cleanup guard before the first fallible reservation. `RunningExecution` and detached activation records carry weak runtime references for abandonment cleanup, with one header registration per execution record. If the runtime has already died, their raw storage is discarded without traversing its heap. Cold publication, eval and native continuation payloads still use public roots and finish outside a held state borrow.

An execution segment borrows `RuntimeState` once and admits its current frame through the private `FrameExecution` lease. Ordinary calls, Base constructors and ordinary returns continue in the same opcode loop. The lease lends the actual current frame and slot transaction; ordinary installation and retirement establish the next current window. Internal ordinary transitions use the carried instruction fallthrough without repeating external PC or window authentication. Capacity, occupancy and dynamic return-target checks remain. Legacy entry still authenticates supplied frame identities and operand witnesses.

State operations in a segment copy, move and release values directly. A selected getter owns its required edges in execution storage before crossing a legacy boundary. Cached native selections contain weak domain/function facts: they apply only to the actual callee, so an outer method selection does not interrupt ordinary argument calls. Cold helpers end the segment's state borrow before they use the public Runtime surface. These remaining boundaries are counted in profiling builds and are migration work, rather than an alternative interpreter.

Ordinary frame installation does not allocate collectible heap nodes and needs no collection poll. A Base constructor publishes its new receiver, arguments and saved return receiver before servicing allocation pressure with the held state. Legacy and outermost completion boundaries retain collection service. Temporary owners remain in state-aware guards until their transfer is complete; cleanup failure quarantines the runtime before further heap traversal.

Direct `Put`, `Set` and initialization use `vm/stack/transfer.rs`. `Put` and initialization transfer the operand owner to the destination. `Set` preserves the operand and obtains an additional owner for the destination. The transfer checks the destination and displaced owner in the active frame window, then either commits there or enters the operation's observation path. Captured and special bindings use their binding operations.

Named reads use the published property site and the selector in `object/ordinary_storage/ic.rs`. The location cache records one or two guarded shapes, an accessor location, or a bounded cooldown; a cold data selection feeds its consumer during the same heap borrow. The selector returns current data, an accessor or complete absence when it can complete the lookup. Ordinary stack reads and borrowed direct-binding reads share that selection. The VM completes admitted data reads in the active frame scope and carries a selected accessor into the property driver. General object semantics remain in the object and property driver modules.

The ready driver materializes the frame for actions classified by `VmAction::observes_activation`: `Call` and `Complete` follow their own frame protocols, while other actions enter the materialization step. An already materialized frame publishes its current PC during that step. Driver operations own the state needed for callbacks, conversions, exceptions and suspension; return paths resume at the published operation-specific position.

## Public roots and panic quarantine

`Runtime` handles share one runtime through `Rc`; cloning a handle does not
copy `RuntimeState`. Heap roots (`ObjectRef`, `CallableRef`, `PropertyKey`,
`SymbolRef`, `Context`, and published bytecode roots) expose fallible
`try_clone` rather than `Clone`. `new_context`, configuration and state
queries also return `Result`. Runtime identity queries and independent
immutable data remain available without accessing heap state.

An unwind across an engine operation marks a `Cell` in the runtime header as
poisoned. `Runtime::is_poisoned` observes that status. Subsequent state entries
return `RuntimeError::Poisoned`; VM adapters report an engine internal error,
so JavaScript cannot catch the corruption and continue execution. A host that
catches a nested panic cannot resume its parent engine operation.

Root, execution and cleanup destructors skip semantic traversal after poison.
On final teardown, `StateStorage` forgets the entire interrupted state, avoiding
a secondary panic from destructors inspecting a partially mutated heap. This
quarantine deliberately leaks the interrupted state. An owned-release failure
also quarantines the runtime, since cleanup may already have changed the heap.
Ordinary JS exceptions and recoverable checked resource errors still follow
normal cleanup and transaction rollback.

## Heap and lifecycle

The heap owns raw records, reference edges, roots, reference counts, zero-count cleanup and cycle collection. Object, Context and FunctionBytecode records occupy the shared arena. Captured cells and shapes have separate typed arenas; String and BigInt records share a leaf arena. Their IDs carry complete generation identities. Storage mutation updates owning edges; semantic object, value and builtin algorithms stay in their respective modules.

Borrowed frame slots and heap references end before a callback, suspension, invalidating mutation or observable cleanup. Active frames, heap records and pending operations transfer ownership across those boundaries. [Typed storage](performance/typed-arenas.md) describes the arena layout and collection lifecycle.

## Language and host boundaries

Compiler constants use primitive values. Live runtime values preserve Object and Symbol identity. Source and module inputs preserve exact bytes and positions; strings preserve UTF-16 code units. Module-host callbacks use their initiating Context and propagate thrown JavaScript values with identity. Jobs own pending computation and retained roots; applications choose when to drain the queue.

The public engine surface is `engine::api`. The CLI and adapters consume that API, and the Test262 runner adds host facilities through its feature-gated integration. The [status page](status.md) lists current language coverage and verification commands.
