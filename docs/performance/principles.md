# Execution principles

These rules apply to every change to the core's loop, heap and builtins. The mechanisms they protect are described in [Execution](execution.md) and [Heap](../heap.md); the core's design record is [`core/README.md`](../../core/README.md).

## The rooting rule

Collection happens only inside an allocating call: a `Heap` method that takes `&mut self` and returns `Result<_, HeapError>`, or anything that calls one (a property operation, a conversion, a call back into JavaScript).

- A borrow of a heap record cannot survive a safepoint, because allocation takes `&mut Heap`.
- An index can. `ObjectId`, `StringId`, `SymbolId`, `BigIntId`, `ScopeId` and any heap `Value` are plain integers; one held in a Rust local across a call that may allocate is made reachable first: pushed on the root stack (`heap.roots().push(v)`, released with `truncate(mark)`), held in a handle, stored in a rooted object, or kept in a register below `top`.
- An allocating call roots its own arguments for its own collection, so a value passed straight into one call needs no rooting. `vm::call` and `vm::construct` write their arguments into the register file before anything allocates.
- The root stack is cut back after every native and every out-of-line iterator operation, so those may push freely.

A missed root fails under the stress mode, where every allocating call collects first and every access to a freed index panics (`cargo test -p oxide-core --features gc-stress`, `scripts/checks/check-gc-stress.py`). Release builds check nothing.

## The `&mut` discipline

`Cx` holds the heap mutably and the code store, realm and host shared. The running unit's code stays borrowed across every heap call: a static unit from the `CodeStore`, a dynamic unit (compiled at run time, owned by the heap) through the `Rc` the loop holds while it runs it, taken out of line at a frame switch so the static path is a plain borrow. `Cx::unit` reads either kind. Nothing that the loop must hold across a call lives in a `CodeUnit` (the caches are in the heap for this reason).

The register file is inside the heap, so the loop's window (`regs`) is a borrow of the heap. An arm that calls into the heap copies its operands out of the window first and takes the window again afterwards; a heap call never receives a register by reference. A hot path that must keep the window across a heap access (the cache hits) takes the heap's arenas as separate fields, disjoint from the register file. Slow paths take `&mut Cx` and register indices, not values borrowed from the window.

The crate has `#![forbid(unsafe_code)]`. The loop indexes registers with plain `[]`: the fixed-size window lets the compiler drop the bounds check on an 8-bit operand, and the publication validator guarantees that every operand lies inside the frame's registers.

## Gates

| Gate | Rule | Read with |
| --- | --- | --- |
| Loop size | `vm::interp::run_at` under 16,384 bytes | `nm --size-sort` on the release `qjs` |
| Instructions | each of the eight V8 v7 programs within 2% of the base's `qjs` in `instructions:u` | `scripts/benchmark/fixed.py --linux-counters` over the [suite matrix](../../scripts/benchmark/README.md) |

A change that crosses the loop-size gate moves code out of line (`rare_op` or another out-of-line arm) before it merges; a new opcode enters `run_at` inline only when a profile of a suite program shows it hot, and is appended at the end of `Opcode`. Both sides of the instruction gate are plain release builds of the same toolchain (`scripts/benchmark/build.py --plain-only`), run with the same manifest, CPU and repeat order (for example `--repeat 4 --order abba --cpu 2`); the medians are compared. A rise past 2% is investigated as a real change before the pull request merges.

## Measurement

Performance conclusions are drawn from retired instructions, not time. Instruction counts are deterministic for a given binary and workload; time also moves with code layout, by 3–6% from layout alone when unrelated code changes. A time change with flat instructions is recorded as layout. Time is reported, with its A/A range, only beside the instruction counts.

Each measurement records the source, the binary's SHA-256, the workload, the machine and the raw samples, and checks every sample's output. The procedure and fields are in [Measurement method](measurement.md); the probes and their Callgrind method in [Diagnostic probes](probes/README.md).
