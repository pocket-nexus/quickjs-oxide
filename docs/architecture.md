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

## VM execution

`src/engine/vm/execute.rs` contains the instruction loop. `FrameCursor` carries the active frame transaction and fault/resume positions. `SlotStore` owns argument, parameter, local and operand windows; `FrameSlots` provides admitted short-lived access. A frame, heap owner or owning pending state roots each value across its lifetime.

Direct `Put`, `Set` and initialization use `vm/stack/transfer.rs`. `Put` and initialization transfer the operand owner to the destination. `Set` preserves the operand and obtains an additional owner for the destination. The transfer checks the destination and displaced owner in the active frame window, then either commits there or enters the operation's observation path. Captured and special bindings use their binding operations.

Named reads use the published property site and the selector in `object/ordinary_storage/ic.rs`. The selector returns current data, an accessor or complete absence when it can complete the lookup. The value is consumed while the heap borrow protects it. The VM completes admitted data reads in the active frame scope and carries a selected accessor into the property driver. General object semantics remain in the object and property driver modules.

The ready driver materializes the frame for actions classified by `VmAction::observes_activation`. Calls and completed frame execution follow their own frame protocol. Driver operations own the state needed for callbacks, conversions, exceptions and suspension; return paths resume at the published operation-specific position.

## Heap and lifecycle

The heap owns raw records, reference edges, roots, reference counts, zero-count cleanup and cycle collection. Object, Context and FunctionBytecode records occupy the shared arena. Captured cells, shapes and leaf values use type-specific storage with complete generation identities. Storage mutation updates owning edges; semantic object, value and builtin algorithms stay in their respective modules.

Borrowed frame slots and heap references end before a callback, suspension, invalidating mutation or observable cleanup. Active frames, heap records and pending operations transfer ownership across those boundaries. [Typed storage](performance/typed-arenas.md) describes the arena layout and collection lifecycle.

## Language and host boundaries

Compiler constants use primitive values. Live runtime values preserve Object and Symbol identity. Source and module inputs preserve exact bytes and positions; strings preserve UTF-16 code units. Module-host callbacks use their initiating Context and propagate thrown JavaScript values with identity. Jobs own pending computation and retained roots; applications choose when to drain the queue.

The public engine surface is `engine::api`. The CLI and adapters consume that API, and the Test262 runner adds host facilities through its feature-gated integration. The [status page](status.md) lists current language coverage and verification commands.
