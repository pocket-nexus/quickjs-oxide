# Current execution paths

This page describes the execution mechanisms in the current source. The main files are `src/engine/compiler/flow.rs`, `src/engine/code/exec.rs`, `src/engine/vm/execute.rs`, `src/engine/vm/stack/transfer.rs`, `src/engine/object/property_ic.rs`, `src/engine/object/ordinary_storage/ic.rs` and `src/engine/vm/property_driver.rs`.

## Compilation and publication

The compiler first generates stack instructions; lowering uses temporary data-flow and effect information to select bounded numeric operations. `verify_parts` (`code/bytecode.rs`) verifies stack states, stack capacity and jump targets; `code/bytecode_validation.rs` authenticates frame, parameter and eval binding layouts before publication; the encoder checks operand counts, instruction widths, boundaries and numeric region plans, then generates a single `ExecCode` word stream. Numeric regions keep the ordinary instructions as the general execution entry after admission. The runtime reads operands, destinations and continuation positions from the published execution words and metadata.

The numeric operations are local accumulation, direct product writes, array element copies, addition after preincremented indexes, combined array/local writes, updates of existing array elements and comparison branches. The selector recognizes the corresponding finite instruction shapes; execution verifies the actual Number, array element and binding states. The success path computes and writes within the admitted frame or array access. Comparisons select the branch position directly.

The publisher also generates fast entries with same-width or bounded spans for some ordinary instructions: local and parameter Number increments, dense Array element reads, array reads and index updates on direct bindings, direct jumps after Number comparisons, and numeric local updates after property or array reads. These entries check values, bindings, array storage and output capacity at runtime; on success they jump to the end of the span, and when admission fails execution continues at the corresponding stack instruction position. `PutArrayEl` writes qualifying scalar elements inside the execution loop and hands other states to property write semantics.

## Ordinary binding writes

`Put`, `Set` and initialization use the ownership-transfer entries of `FrameSlots`. `Put` and initialization consume the operand owner; `Set` keeps the operand and obtains the owner the destination needs. The target slot and operand are verified within one borrow of the frame window. The target's previous value is uninitialized, an ownerless scalar or an owned value; its release requirement decides between local completion and an observation boundary. Direct parameter and local bindings share the same transfer implementation. Lexical states, captured bindings and special bindings are handled by the corresponding semantic operations.

## Named property reads

Published property sites hold a static key and a read cache (`object/property_ic.rs`). `ordinary_storage/ic.rs` selects a data value, an accessor or a complete miss from the current object and its prototype chain, and updates the location cache where applicable. The cache records a monomorphic location or a polymorphic set of at most four, and also records accessor locations; a site over capacity retries recording after a bounded cooldown. A hit reads the object's current slot value. On a cold site, the data value selected during traversal is passed directly to the consumer. A complete miss is consumed from the result of that lookup.

Ordinary `GetField`, `GetField2` and the `BorrowedField` entries on direct local or parameter bindings reuse this selection flow. Data values are copied or take an owner during the heap borrow; borrowed bindings keep the receiver in the original binding. Array index reads use a separate dense-element admission path.

`execute_frame` completes qualifying named data reads within the currently authenticated frame window. A selected accessor is handed to the property driver as owned state; general property semantics, callbacks and observable cleanup continue in the driver. After the driver returns, execution continues at the corresponding fault, fallthrough or resume position.

## Frames, ownership and lifetime

`SlotStore` provides windows for parameters, locals and operands; `FrameTransaction` and `FrameSlots` scope short accesses. Call inputs, frame functions and capture storage hold raw heap IDs and are retired explicitly with the current state; abandoned execution is covered by Weak registration. A span acquires `RuntimeState` once and borrows the actual current frame and window through `FrameExecution`. Ordinary calls, returns, tail retirement and Base construction continue in the same opcode loop, using the carried fallthrough and window facts and keeping capacity, occupancy and dynamic destination checks.

Pending operations hold the owners needed across boundaries. Object, native, eval, publication and suspension helpers run after the span borrow ends; profiling builds count these boundaries. The ready driver materializes for actions selected by `VmAction::observes_activation`; a materialized frame publishes its current PC at this step. The public API acquires state and authenticates external roots; a panic or a failed destructive cleanup isolates the runtime, and later entries return `RuntimeError::Poisoned`.

Captured variables and shapes are stored in separate typed arenas. Each ID carries a full generational identity; reference release, the zero-count queue and explicit collection follow the same heap lifetime rules. The storage contract is described in [Typed storage](typed-arenas.md).

## Automatic cycle collection

Cycle node publication consumes a unified net-growth budget, and ordinary refcount cleanup returns budget in batches. Allocation sites that hold state (object and array literals, Base construction, property writes that publish a new layout, and similar) service collection requests with the held state after publishing all owners; the ready driver and the outermost execution turn service requests at public boundaries. Explicit and automatic GC use the same full collector. Budget, weak roots, reentrancy and borrowing rules are in [GC policy](../cycle-collection.md).

## Verification

- Numeric selection and fallback: `src/engine/compiler/tests/numeric_region.rs` and the VM numeric tests.
- Ordinary binding transfer: tests in `src/engine/vm/stack/transfer.rs`.
- Property selection and completion: tests in `src/engine/object/property_ic.rs`, `src/engine/object/ordinary_storage/ic.rs`, `src/engine/vm/stack.rs` and `src/engine/vm/property_driver.rs`.
- Global semantics: `cargo test --locked --workspace --all-targets` and the [Test262 guide](../test262.md).
- Cost measurement: [Measurement method](measurement.md).
