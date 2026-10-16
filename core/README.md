# oxide-core

The new core of quickjs-oxide (issue #122), built in phases next to the current
engine. Phase 1 is the value and heap layer; phase 2 adds register bytecode
and its main loop. As of phase 2 M1 the instruction set, code units and their
validator exist, but nothing lowers to them or runs them yet, and no builtin
exists. The plan is [`docs/performance/new-core-phase1.md`](../docs/performance/new-core-phase1.md);
the measurements are in [`docs/performance/new-core-phase1-results.md`](../docs/performance/new-core-phase1-results.md);
phase 2 is planned in [`docs/performance/new-core-phase2.md`](../docs/performance/new-core-phase2.md).

The crate has `#![forbid(unsafe_code)]` and no dependencies. From phase 2 the
root crate depends on it (the register lowering lives there); never the
reverse.

## Layout

| Module | Content |
| --- | --- |
| `value` | `Value(u64)` (NaN-boxed, doubles favored), `Kind`, identifier newtypes |
| `heap::arena` | `Arena<T>`: `Vec<T>`, free list, side mark and live bitmaps |
| `heap::values` | the value heap: power-of-two ranges over one `Vec<Value>` |
| `heap::object` | `ObjectHeader` (20 bytes), `ObjectKind`, payload arenas |
| `heap::string`, `atom`, `symbol`, `bigint` | leaf records, interning, `PropertyKey` |
| `heap::gc` | root stack, handles, mark-sweep, trigger, stress mode, heap cap |
| `shape` | transition tree, dictionary mode, validity cells |
| `bytecode` | `Insn` (8 bytes), `Opcode` and its operand formats, `CodeUnit`, `CodeStore`, the builder, the publication validator, the disassembler |
| `runtime` | `Runtime`: the heap, the code store beside it, and the single realm |

## The rooting rule

Collection happens only inside an allocating call (any `Heap` method that
takes `&mut self` and returns `Result<_, HeapError>`), never elsewhere.

- Allocation takes `&mut Heap`, so a *borrow* of a record can never survive a
  safepoint.
- An *index* can: `ObjectId`, `StringId`, `SymbolId`, `BigIntId`, `ScopeId` and
  any heap `Value` are plain integers. Any of them held in a Rust local across
  a call that may allocate must be reachable from a root first: pushed on the
  `RootStack` (`heap.roots().push(v)`, released with `truncate(mark)`), held in
  a handle (`heap.handles().create(v)`), or stored in an object that is itself
  rooted.
- An allocating call roots its own arguments for its own collection, so a value
  passed straight into one call needs no rooting. Atoms are strong roots in
  phase 1; symbols, BigInts and plain strings are not.

Code that breaks the rule fails under the stress mode:

```sh
cargo test -p oxide-core --features gc-stress
```

With `gc-stress` every allocating call collects first, sweep poisons freed
records, freed indices are quarantined for one collection so they are not
immediately recycled, and every access asserts liveness: a missed root panics
with `stale … index` at the access that used it. After each collection the
verifier re-marks from the roots and checks that live and reachable records
agree, that no free-list entry is live or marked, and that every value-heap
range has exactly one owner. Debug builds assert liveness too; release builds
check nothing.

## Departures from the plan's sketch

- The root stack, handle table and fixed roots live inside `Heap` rather than
  beside it in `Runtime`, so a safepoint in `Heap::alloc_*` sees them without
  extra parameters. `Runtime` holds the heap and the realm.
- The special values use payload bits 0–1 for the kind (Bool, Undefined, Null,
  Empty) and bit 2 for the boolean, so `kind()` is a branchless table lookup.
- Every value-heap range is a power of two, including large ones, so a header
  stores each range as a start and a `u8` exponent and stays at 20 bytes.
- `delete_property`, `set_prototype` and `set_property_flags` return
  `Result`: entering dictionary mode or replaying a shape allocates.
- Array indices from 2^31 to 2^32 − 2 are atoms whose record caches the index,
  as in the current core.

## Phase 2 departures

- Compare-jumps against a literal (`JLtI` and friends) carry an `i16` in
  `b | c << 8` and the absolute target in `imm`: an `i32` literal and an
  absolute target do not both fit. The lowering materializes a literal outside
  the `i16` range. The arithmetic immediates (`AddI`, …) keep an `i32` in `imm`.
- The relational compare-jumps come with negated forms (`JNlt`, `JNlte`,
  `JNgt`, `JNgte`): `if (!(a < b))` is not `a >= b` when an operand is NaN.
- A handler entry stores the register the thrown value lands in (`T(depth)`)
  rather than the depth.
- `CheckInit` carries the binding's key for the `ReferenceError` message.
- `NewArray a, b, imm` takes its elements from the `imm` registers starting at
  `b`, where the stack code's `ArrayFrom` left them, instead of a
  `DefineElem` per element.
- There is one `FunctionBytecode` record per published code unit, shared by
  every closure over it. It holds the constant table and the child records'
  indices (as `Int32` values in a second value-heap range), so a live function
  keeps alive every function it can create. A tree is created under one
  safepoint by `Heap::alloc_bytecode_function` (via `Runtime::instantiate`),
  because a record is not a value the root stack can hold; `MakeClosure` uses
  `Heap::alloc_closure`, whose record is reachable through the running
  function. This replaces phase 1's `attach_bytecode`.
