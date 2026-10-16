# New core, phase 1: value and heap

This page is the implementation plan for phase 1 of the new core tracked in issue #122 ("8-byte `Value`; typed arenas; value heap with range allocation; mark-sweep with allocation safepoints; `&mut Runtime` threading; shapes with transition tree, dictionary mode and validity guards"). It is written for whoever implements the phase: it fixes the decisions that are already made, lists the ones that are deliberately open, and gives each milestone an acceptance check that can be run without judgment calls. Day estimates order the work; they are not commitments.

Read first: [#122](https://github.com/pocket-nexus/quickjs-oxide/issues/122) for the overall design and phase order, [Measurement method](measurement.md) for how numbers are produced, [Typed storage](typed-arenas.md) for what the current heap does, and `docs/deviations.md` for how feature drops are recorded.

## 0. Scope

**In scope.** The `Value` representation; the heap (typed arenas, value heap, mark-sweep collector, root set, handles); `&mut Runtime` threading; shapes (transition tree, dictionary mode, validity cells); storage for strings, atoms, symbols and BigInts; a GC stress mode; a heap cap.

**Out of scope.** No bytecode is consumed and no JavaScript runs. `compiler`, `regexp`, `source` and `modules` are untouched. No builtin is ported. Every "program" in phase 1 is a Rust test or a Rust-driven micro-workload.

Why the phase is cut here: the earlier hot-loop attempt stayed at about 61 instructions per dispatch because operands were still 16-byte, reference-counted, non-`Copy` slots. A register VM only pays off once operands are plain `Copy` words, so the foundation is built and measured before phase 2 starts (#122, "Why this order").

## 1. Location: a separate crate `core/`

A new workspace member `core/`, package `oxide-core`, with `#![forbid(unsafe_code)]` at the crate root. In phase 1 it has no dependency on the root crate `quickjs-oxide`, and the root crate does not depend on it.

- Phase 2 adds the dependency root crate → `oxide-core` and writes the lowering from the compiler's IR to the new register bytecode inside the root crate, which is the only place that sees both type systems.
- A separate crate makes three things true: the old `JsValue`/`ObjectId` cannot be used by accident; `cargo test -p oxide-core` builds in seconds instead of the root crate's minutes; `forbid(unsafe_code)` is a crate-level assertion matching the "safe Rust" commitment in #122.
- Whether `oxide-core` folds back into the root crate when the old core is deleted (phase 6) is decided then.

Module layout, one file or directory each:

| Module | Content |
| --- | --- |
| `value` | `Value(u64)`, encoding and decoding, `Kind` |
| `heap::arena` | `Arena<T>`: `Vec<T>` plus a free list and a side mark bitmap |
| `heap::values` | the value heap: a range allocator over one `Vec<Value>` |
| `heap::object` | `ObjectHeader`, `ObjectKind`, kind-specific payload arenas |
| `heap::string`, `heap::atom`, `heap::symbol`, `heap::bigint` | leaf records and interning |
| `heap::gc` | root set, mark stack, sweep, trigger policy, stress mode, heap cap |
| `shape` | shape records, transition tree, dictionary mode, validity cells |
| `runtime` | the `Runtime` struct, realm skeleton, handle table |
| `benches/` or `examples/` | the Rust micro-workloads of M5 |

Delivery: one pull request per milestone, stacked in milestone order, each with its acceptance check in the description. `cargo clippy -p oxide-core --all-targets -- -D warnings` and `cargo fmt --check` gate every one. Toolchain is the pinned mainline Rust from `rust-toolchain.toml`.

## 2. `Value`

```rust
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Value(u64);

pub enum Kind { Number, Int32, Bool, Undefined, Null, Empty, Object, String, Symbol, BigInt }
```

`Copy`, no `Drop`, no ownership protocol.

**Encoding: NaN-boxing, doubles favored.** Any bit pattern outside the boxed range is an `f64`. The boxed range is the quiet-NaN space with the sign bit set; the top 16 bits select the kind and the low 32 bits carry the payload:

| Top 16 bits | Kind | Payload |
| --- | --- | --- |
| `0xFFF9` | Int32 | `i32` |
| `0xFFFA` | Bool, Undefined, Null, Empty | 0..3 |
| `0xFFFB` | Object | `u32` object index |
| `0xFFFC` | String | `u32` string index |
| `0xFFFD` | Symbol | `u32` symbol index |
| `0xFFFE` | BigInt | `u32` BigInt index |
| `0xFFFF` | reserved | |

- A real NaN is canonicalized to `0x7FF8_0000_0000_0000` on construction, so `is_number()` is one comparison (`bits < 0xFFF9_0000_0000_0000`) and `kind()` is one shift.
- `Empty` is the internal hole marker (array holes, TDZ-uninitialized bindings, empty hash slots). It is never a JavaScript value. `Option<Value>` is not used anywhere: `Value` has no niche, so `Option<Value>` would be 16 bytes.
- Payloads are indices, not pointers, so the encoding is the same on 32-bit targets (PSP/MIPS) and heap snapshots need no pointer rewriting (the phase 5 prerequisite).
- Dropped: the current `JsValue` keeps `ShortBigInt(i64)` inline. An `i64` does not fit, so every BigInt is a heap record. The `bigint_32`/`bigint_64` fixed probes will be slower until phase 4 gives small BigInts a single-limb record. Phase 1 only records the decision.

**Identifiers.** `ObjectId(u32)`, `StringId(u32)`, `ShapeId(u32)`, `SymbolId(u32)`, `BigIntId(u32)`, `ScopeId(u32)`, `ValidityCellId(u32)`: newtypes with no generation. Release builds check nothing. Debug builds keep a `live: Vec<bool>` per arena and assert liveness on every access. A stale index is a logic error caught by the stress mode and the debug assertions; this is the mitigation #122 names under "Notes on the GC discipline".

**Interface sketch.**

```rust
impl Value {
    pub const UNDEFINED: Value; pub const NULL: Value; pub const TRUE: Value;
    pub const FALSE: Value; pub const EMPTY: Value;
    pub fn number(f64) -> Value;          // canonicalizes NaN; does not try to make an Int32
    pub fn int32(i32) -> Value;
    pub fn object(ObjectId) -> Value;     // and string/symbol/bigint
    pub fn kind(self) -> Kind;
    pub fn is_number(self) -> bool;       // Int32 or double
    pub fn as_f64(self) -> Option<f64>;   // Int32 widened
    pub fn as_int32(self) -> Option<i32>;
    pub fn as_object(self) -> Option<ObjectId>;  // and string/symbol/bigint
    pub fn is_empty(self) -> bool;
}
```

**Acceptance (M1).** Round-trip tests for every kind; ±0, several NaN bit patterns, ±inf and subnormals; `const` assertions `size_of::<Value>() == 8` and `size_of::<[Value; 4]>() == 32`; a test that `Value::number(f64::NAN).0 == 0x7FF8_0000_0000_0000`; an inspection of the release assembly of `kind()` and `as_object()` recorded in the PR (shift and compare, no branch chain).

## 3. Heap

```rust
pub struct Heap {
    objects: Arena<ObjectHeader>,   // one fixed-size record per object
    shapes:  Arena<Shape>,
    strings: Arena<StringRecord>,
    symbols: Arena<SymbolRecord>,
    bigints: Arena<BigIntRecord>,
    values:  ValueHeap,             // every property value, element and scope slot
    payloads: PayloadArenas,        // kind-specific payloads
    marks:   MarkBits,              // one bitmap per arena
    gc:      GcState,               // thresholds, counters, stress flag, cap
}
```

**`Arena<T>`.** A `Vec<T>` with a free list. The free link lives in the record itself when `T` has a natural free variant, otherwise in a parallel `Vec<u32>`; pick per type. No reference counts, no generations, no zero-count queue.

```rust
impl<T> Arena<T> {
    pub fn alloc(&mut self, value: T) -> u32;     // reuses the free list first
    pub fn get(&self, index: u32) -> &T;          // debug: asserts live
    pub fn get_mut(&mut self, index: u32) -> &mut T;
    pub fn free(&mut self, index: u32);           // sweep only
    pub fn live_count(&self) -> usize;
    pub fn bytes(&self) -> usize;                 // for the cap and the diagnostics
}
```

**`ValueHeap`.** One `Vec<Value>` allocated in ranges `Range { start: u32, len: u32 }`. Size classes 1, 2, 4, 8, …, 256 each keep a free list; larger ranges use a separate large-block list. Property values, dense array elements and scope slots are ranges, so an object is one header plus one range, a property hit is `values[start + slot]`, and there is no per-object `malloc`. Sweep returns the ranges of dead objects to their size class.

```rust
impl ValueHeap {
    pub fn alloc(&mut self, len: u32) -> Range;          // filled with Value::EMPTY
    pub fn grow(&mut self, range: Range, new_len: u32) -> Range;  // alloc, copy, free
    pub fn free(&mut self, range: Range);
    pub fn get(&self, range: Range, slot: u32) -> Value;
    pub fn set(&mut self, range: Range, slot: u32, value: Value);
}
```

- Growth (a property beyond the reserved range, a `push` past the elements range) allocates a new range, copies and frees the old one. Pre-sizing the property range from a constructor's `this.x =` count is phase 3 work; phase 1 exposes `with_capacity`.
- Risk: fragmentation under long churn. Phase 1 does not compact. M5 measures the fragmentation ratio on a churn workload; if it is unacceptable, the fallback is one `Vec<Value>` per object (one allocation each) behind the same interface.

**`ObjectHeader`** (target 24 bytes or less):

| Field | Type | Notes |
| --- | --- | --- |
| `shape` | `ShapeId` | the prototype lives in the shape |
| `slots` | `Range` | named property values |
| `elements` | `Range` | dense elements; a hole is `Value::EMPTY` |
| `kind` | `ObjectKind` (`u8`) | phase 1 defines Ordinary, Array, Function, Scope, GlobalObject, Native (placeholder) |
| `flags` | `u8` | extensible, used_as_prototype, callable, constructor, dictionary |
| `payload` | `u32` | index into the kind's side arena, or an inline small payload such as the array length |

The current `ObjectKind` has about forty variants. Their payloads are added as side arenas per kind when the builtins are ported in phase 4; the header does not change.

**Scopes.** `Scope` is an `ObjectKind`: the `slots` range holds the variables and `payload` is the parent `ScopeId`. A closure captures by holding a `ScopeId`, the collector traces scopes like any object, and `LoadScope(depth, index)` is `depth` parent hops plus one load. The per-variable reference-counted cells of the current core have no counterpart.

**Strings, atoms, symbols, BigInts.**

- `StringRecord { repr: OneByte(Box<[u8]>) | TwoByte(Box<[u16]>), hash: u32 (lazy), is_atom: bool }`. Phase 1 has flat strings only; ropes come in phase 4.
- Atom table: a `HashMap` from string content to `StringId`. In phase 1 atoms are strong roots, which is simple and testable; M4 records the table's growth under a Test262-sized key set, and the switch to a weak table (rebuilt after sweep) is decided on that number.
- `PropertyKey(u32)` keeps the current `Atom` trick: a high-bit tag separates array indices from atom and symbol indices, and array indices never enter the atom table.
- `BigIntRecord { sign, limbs: Box<[u64]> }`. Phase 1 defines the record and its tracing only.

## 4. Garbage collection

**Algorithm.** Non-moving mark-sweep. Mark bits live in a side bitmap per arena (`Vec<u64>`); marking uses an explicit `Vec<Value>` stack, never recursion. Sweep is eager and scans every arena; lazy sweeping is an optional later change, decided on M5 pause data.

**Root set.**

1. Fixed roots in `Runtime`: each realm's global object and intrinsic table, the atom table, the job queue, the module registry.
2. `RootStack: Vec<Value>` with scoped marks. This is the only legal way for Rust code in phase 1 to hold a value across an allocation. The register file of phase 2 becomes another root.
3. `Handles: Vec<Value>` plus a free list, for values held by the embedder (`api::Value`, the CLI, the Test262 host), released explicitly.
4. Pending native state machines (phase 4) implement a `Trace` trait and report their values to the marker.

```rust
impl RootStack {
    pub fn mark(&self) -> RootMark;
    pub fn push(&mut self, value: Value);
    pub fn truncate(&mut self, mark: RootMark);
}
```

**Safepoints.** Only inside `Heap::alloc_*`, and only when `bytes_since_gc >= threshold`, with `threshold = max(min_heap, live_after_last_gc * growth_factor)`. Nothing else triggers a collection.

**Stress mode.** `GcState.stress: bool`: when set, every allocation collects. CI runs the whole `oxide-core` test suite a second time with the stress feature enabled (`cargo test -p oxide-core --features gc-stress`). Its purpose is to turn stale-index bugs from intermittent into deterministic.

**Discipline rule** (also goes into `core/README.md`). Allocation takes `&mut Heap`, so a borrow of a record can never survive a safepoint. An index can, so any `ObjectId`, `StringId` or `Value` held in a Rust local across a call that may allocate is pushed on the `RootStack` first. Code that breaks the rule fails under stress mode.

**Weak references.** `WeakRef`, `WeakMap` and `FinalizationRegistry` semantics are phase 4. Phase 1 keeps one hook: a weak table processed after marking.

**Heap cap.** `HeapLimits { max_bytes }`. An allocation that would cross the cap collects first; if the collection does not free enough, `alloc_*` returns `Err(OutOfMemory)`, which the caller turns into the JavaScript `InternalError`. Arena growth and value-heap growth both count.

## 5. Shapes

```rust
pub struct Shape {
    parent: Option<ShapeId>,            // transition-tree parent
    key: PropertyKey, flags: PropertyFlags, slot: u32,   // the property this node adds
    count: u32,                         // properties up to and including this node
    prototype: Option<ObjectId>,
    transitions: Transitions,           // Vec for up to 4 entries, HashMap beyond
    table: Option<Box<HashMap<PropertyKey, u32>>>,   // built lazily past 8 properties
    validity: ValidityCellId,
}
```

- The transition tree has one root per `(prototype, kind)`. Lookup uses `table` when it exists and otherwise walks `parent` linearly; for eight properties or fewer that is cheaper than hashing.
- Dictionary mode gives an object an unshared shape with its own hash table and insertion-order links, and stops producing transitions. Entered on property deletion, past a property-count threshold, and for the global object.
- Validity cells: every object used as a prototype owns a `ValidityCell { valid: bool }`. Any change to a prototype object (property added, deleted or reflagged, `[[Prototype]]` changed) clears its own cell and, through a dependents list of prototype objects that inherit from it, the cells downstream. The prototype IC of phase 3 records `(holder ObjectId, slot, ValidityCellId)` and a hit tests one boolean: no chain walk, no global epoch. Phase 1 implements the cell arena, the dependents list and `invalidate`, with a test for transitivity.
- The tests of the current `src/engine/object/shape.rs` (transitions, dictionary order, prototype edges, duplicate keys) are ported as M3's acceptance.

```rust
impl Heap {
    pub fn add_property(&mut self, object: ObjectId, key: PropertyKey, flags: PropertyFlags, value: Value) -> Result<(), HeapError>;
    pub fn lookup_own(&self, object: ObjectId, key: PropertyKey) -> Option<(u32, PropertyFlags)>;
    pub fn get_slot(&self, object: ObjectId, slot: u32) -> Value;
    pub fn set_slot(&mut self, object: ObjectId, slot: u32, value: Value);
    pub fn delete_property(&mut self, object: ObjectId, key: PropertyKey) -> bool;   // enters dictionary mode
    pub fn set_prototype(&mut self, object: ObjectId, prototype: Option<ObjectId>);  // invalidates
}
```

## 6. `&mut Runtime`

```rust
pub struct Runtime {
    heap: Heap,
    realms: Vec<Realm>,     // phase 1: global object and intrinsic slots only
    roots: RootStack,
    handles: Handles,
    limits: HeapLimits,
}
```

Every operation is `fn(&mut Runtime, …)`. There is no `Rc<RefCell<RuntimeState>>`, no `try_borrow_mut`, no poisoned bit. A native (phase 4) receives `&mut Runtime` and its own state; to call back into JavaScript it returns a step to the VM, so no borrow is ever held across a JavaScript call. That is what makes a single `&mut` workable with the existing Step/Resume natives, and the reason that design is kept.

The embedder's handle shape (scoped `Local` handles versus explicitly released `Persistent` ones) depends on how `apps/cli`, `conformance/test262` and `adapters/*` use the API today (`Runtime`, `Context`, `Value`, `JsString`, `ObjectRef`, `PropertyKey` and the module-loading types). That decision belongs to phase 4, when `api` moves. Phase 1 only guarantees that the handle table exists and is traced.

## 7. 32-bit targets and memory

Every index is a `u32`; `Value` stays a `u64` on 32-bit targets (two registers, accepted). Arenas and the value heap grow by doubling under `HeapLimits`. Phase 1 builds no snapshots, but nothing in the design holds a pointer that a snapshot would have to rewrite.

## 8. Milestones

| # | Content | Acceptance | Estimate |
| --- | --- | --- | --- |
| M1 | crate skeleton, `Value`, `Kind`, identifier newtypes | the checks listed in section 2 | 1–2 days |
| M2 | `Arena<T>`, mark bits, `RootStack`, handles, mark-sweep, stress mode, `HeapLimits` | cycles are collected; rooted values survive; a released handle's value is collected; free-list reuse is deterministic; the whole suite passes under `gc-stress`; the `OutOfMemory` path has a test | 3–5 days |
| M3 | value-heap range allocator, `ObjectHeader`, property add/lookup/set/delete, transition tree, dictionary mode, dense arrays with holes, the `Scope` kind, validity cells | the ported `shape.rs` tests pass; the instruction count of the property-hit path is recorded from the assembly; validity transitivity test | 5–7 days |
| M4 | one-byte and two-byte strings, atom table, `PropertyKey`, symbols, BigInt records | interning equivalence; index keys stay out of the atom table; atoms survive collection; atom-table size recorded for a Test262-sized key set | 2–3 days |
| M5 | Rust micro-workloads and reference-machine measurement | the table below is committed under `docs/performance` | 2 days |
| M6 | a churn run under a 32 MB cap | stable RSS, no `OutOfMemory`, pause times recorded | 1 day |

**M5 workloads.** Each one maps to a probe the current core already has, so the comparison is construct by construct:

| Workload | Current-core counterpart | Compared quantity |
| --- | --- | --- |
| constructor-style allocation: N times "new object, then four sequential property adds" | `allocation/ledger.py`'s `new E()`; the M1 probe `alloc_small` (4345 instructions per iteration including dispatch) | instructions per allocation, against the ledger's allocation phase (dispatch excluded) |
| literal-style allocation: reserve four slots, then fill | the ledger's object-literal probe | same |
| closures: per iteration, one `Scope` plus one function object holding it | the `instantiate_closure` exits reported on #126 for Havlak and DeltaBlue | instructions per iteration and collector load |
| prototype-chain read: a shape lookup at depth 3 | the `prop_read` fixed probe | instructions on the hit path |
| churn: allocate and drop, 10⁷ times | none | collector throughput (MB/s), fragmentation ratio, peak RSS |

Measurement follows [Measurement method](measurement.md): reference machine, serial runs, raw samples kept. Instruction counts come from `perf stat` or Callgrind. The exit condition of phase 1 is this table plus the M6 result committed under `docs/performance`; that table decides whether phase 2 starts on the #122 design. No target multiplier is written here; the number is measured first, and the go/no-go gate of #122 is in phase 3.

Total: about 15–20 working days.

## 9. Decisions

Made, and not reopened inside phase 1:

- separate crate `oxide-core`, `forbid(unsafe_code)`, no dependency edge to the root crate;
- NaN-boxed `Value(u64)`, doubles favored, 32-bit index payloads, `Empty` instead of `Option<Value>`;
- identifiers without generations; stress mode and debug liveness assertions as the mitigation;
- value heap with size-class ranges; `Scope` as an object kind;
- collection only at allocation; eager sweep;
- validity cells with a dependents list;
- atoms strong in phase 1.

Open, with the milestone that decides them:

- value-heap fragmentation versus one `Vec<Value>` per object: M5 churn;
- strong versus weak atom table: M4 size record;
- eager versus lazy sweep: M5 pause data;
- the embedder's handle shape: phase 4;
- single-limb small BigInts: phase 4.

## 10. Risks

- **Value-heap fragmentation.** Decided by M5; the interface does not change either way.
- **Strong atoms.** A full Test262 run accumulates many atoms; M4 records the size.
- **Identifiers without generations.** A stale index is no longer caught by the type system or a runtime check. Mitigations: stress mode in CI, debug liveness assertions, the `RootStack` rule. This is the trade-off #122 accepted; this page makes the mitigations concrete.
- **`ShortBigInt` lost.** The `bigint_*` probes regress until phase 4.
- **Validity-cell dependents lists.** With many prototype objects (one per class) list maintenance has a cost; phase 3 checks it on the real benchmarks, phase 1 only guarantees correctness.
- **Embedder API shape.** Decided in phase 4.

## 11. Seam with the current core (phase 2 preview, not phase 1 work)

The compiler produces `UnlinkedFunction`: stack instructions (`Vec<Instruction>`, about 180 opcodes), constants, variable definitions, closure variables and numeric regions, with a 42-variant `IrOp` IR upstream. Phase 2 allocates registers from the IR inside the root crate and lowers to the new core's fixed 8-byte instructions, bypassing the stack instructions. The only thing phase 1 reserves for it: `FunctionBytecode` is an arena record whose constant table is a range in the value heap, so phase 2's code units are traced like everything else.
