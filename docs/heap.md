# The heap and the collector

This page describes the heap of `oxide-core` (`core/src/heap/`, `core/src/shape.rs`, `core/src/value.rs`): how values are represented, where records live, how shapes and validity cells keep inline caches valid, and how the mark-sweep collector finds its roots, when it runs, and what the memory cap does. [`core/README.md`](../core/README.md) is the core's design record and states the rooting rule in full. [Planned work](planned.md) extends this design with a cap-aware trigger, growth that stops at the cap, shrinking after a collection, reclaiming dynamic code and heap snapshots.

## Values

`Value` is a `u64` (`#[repr(transparent)]`, `Copy`, no `Drop`), NaN-boxed with doubles favored. Every bit pattern below `0xFFF9_0000_0000_0000` is an `f64`; the boxed range is the quiet-NaN space with the sign bit set, where the top 16 bits select the kind and the low 32 bits carry the payload.

| Top 16 bits | Kind | Payload |
| --- | --- | --- |
| `0xFFF9` | Int32 | `i32` |
| `0xFFFA` | Bool, Undefined, Null, Empty | bits 0–1: 0 Bool, 1 Undefined, 2 Null, 3 Empty; bit 2: the boolean |
| `0xFFFB` | Object | `u32` object index |
| `0xFFFC` | String | `u32` string index |
| `0xFFFD` | Symbol | `u32` symbol index |
| `0xFFFE` | BigInt | `u32` BigInt index |
| `0xFFFF` | reserved | |

- `Value::number` keeps a NaN's bits, as QuickJS's unboxed doubles do, unless they fall in the boxed range; those become `0x7FF8_0000_0000_0000`. It never produces an Int32.
- `kind()` is a branchless table lookup; `is_number()` is one comparison.
- `Value::EMPTY` is the internal hole marker: array holes, bindings in the temporal dead zone, deleted table entries, released handles. It is never a JavaScript value, and `Option<Value>` is not used.
- Payloads are 32-bit arena indices, not pointers, so the encoding is the same on 32-bit targets and a snapshot would need no pointer rewriting.
- Every BigInt is a heap record (`BigIntRecord`: a sign and little-endian `u64` limbs); an `i64` does not fit the payload.

The identifiers (`ObjectId`, `StringId`, `SymbolId`, `BigIntId`, `ShapeId`, `ScopeId`, `ValidityCellId`, `BytecodeId`) are `u32` newtypes without a generation. A stale index is a logic error; the rooting rule, the debug liveness assertions and the stress mode are what catch it.

## Arenas

`Arena<T>` (`heap/arena.rs`) is a `Vec<T>` with a free list, a side mark bitmap and a side live bitmap. A freed slot is overwritten with the type's `vacant()` poison record. Sweep pushes freed indices in descending order, so the lowest free index is reused first and reuse is deterministic. Debug and `gc-stress` builds assert on every access that the index is live; release builds check nothing. The record vector grows by doubling (`reserve_exact` of the current capacity, at least 4) and never shrinks.

The heap (`Heap` in `heap/mod.rs`) has nine arenas and the value heap:

| Arena | Record | Content |
| --- | --- | --- |
| `objects` | `ObjectHeader` (20 bytes) | one header per object |
| `shapes` | `Shape` | transition-tree nodes and dictionary shapes |
| `strings` | `StringRecord` (at most 40 bytes) | flat strings, ropes and atoms; contents out of line |
| `symbols` | `SymbolRecord` | the description's string index and a private-name flag |
| `bigints` | `BigIntRecord` | sign and limbs |
| `cells` | `ValidityCell` | one `valid` boolean |
| `payloads.functions` | `FunctionRecord` | a function's scope, bytecode or native, home object, field initializer |
| `payloads.bytecode` | `FunctionBytecode` | per published code unit: its constant table and child records, both value-heap ranges |
| `payloads.internals` | `Internal` | the internal slots and non-value data of the builtin kinds |

A record's out-of-line bytes (string contents, BigInt limbs, internal slot vectors) are reported through `Record::extra_bytes` and count toward the heap's size. The code store (lowered code units, their position tables and source text) and the regular-expression programs live beside the heap in `Runtime`: they are not collected and do not count toward the cap.

## The value heap

`ValueHeap` (`heap/values.rs`) is one `Vec<Value>` handed out in ranges. Every property value, dense element, scope variable, constant table and child list is a range. Every range is a power of two (classes 1, 2, 4, … up to 2³¹), each class keeps its own free list, and a freed range is filled with `Value::EMPTY` and pushed on its class's list. Growing a range allocates the next class, copies and frees the old one. Ranges never move: under allocation churn the free share is bounded by the collection trigger rather than by fragmentation.

Because a range's length is always its class size, an object header stores each range as a start and a `u8` exponent.

## Objects

`ObjectHeader` (`heap/object.rs`):

| Field | Type | Content |
| --- | --- | --- |
| `shape` | `ShapeId` | the property layout; the prototype lives in the shape |
| `slots_start`, `slots_log` | `u32`, `u8` | the range of named property values |
| `elements_start`, `elements_log` | `u32`, `u8` | the range of dense elements; a hole is `Value::EMPTY` |
| `payload` | `u32` | an index into the kind's side arena, or an inline payload (an array's length, a scope's parent) |
| `kind` | `ObjectKind` (`u8`) | |
| `flags` | `u8` | extensible, used as prototype, callable, constructor, dictionary, error, sparse, read-only `length` |

The kinds are `Ordinary`, `Array`, `Function` (payload: a `FunctionRecord`), `Scope` (slots: the variables; payload: the parent scope), `GlobalObject` (always in dictionary mode), `Arguments`, `RegExp`, `ForInIterator`, `Native`, and the kinds with an `Internal` record: the five primitive wrappers, `Date`, `BoundFunction`, `Proxy`, `TypedArray`, `ModuleNamespace`, and `Builtin`, whose `InternalTag` names the builtin (iterators, promises, collections, suspended generator frames and the like). A captured binding lives in a cell, a one-slot `Scope` object, and a closure holds one cell per captured binding ([`core/README.md`](../core/README.md#bindings-and-cells)).

An object with the `SPARSE` flag keeps its index properties as named properties and has an empty element range; the flag is set when an element gets non-default attributes or the object stops being extensible.

## Strings, atoms and property keys

- A `StringRecord` is flat or a rope. The flat representation is canonical: `OneByte(Box<[u8]>)` whenever every code unit fits in a byte, otherwise `TwoByte(Box<[u16]>)`, so equal contents have equal representations. A rope is a concatenation whose halves live in `Heap.ropes`, keyed by the record; it flattens on the first use of its units, and its entry is dropped at the next collection after it flattens or dies. The content hash is computed on first use.
- An atom is a string record with `is_atom` set. The atom table maps a content hash to an atom index (collisions in a side list) and compares by content against the string arena, so an entry is 8 bytes and holds no second copy of the string. Atoms are strong: every collection marks every atom. The 99th-percentile Test262 test interns 194 atoms, and one runtime running every file stays at 16 MiB.
- `PropertyKey(u32)`: bit 31 set marks an immediate array index below 2³¹, which never enters the atom table; otherwise bit 30 set marks a symbol, and a clear bit 30 an atom. Array indices from 2³¹ to 2³² − 2 are atoms whose record caches the index, so key ordering still treats them as indices. String and symbol indices used as keys must stay below 2³⁰; interning past that fails with `OutOfMemory`.

## Shapes and validity cells

`shape.rs` holds the shapes. A transition shape describes one property (key, flags, slot) added on top of its parent; the tree has one root per `(prototype, kind)`, found through the weak `shape_roots` table with a one-entry cache in front. Transitions are a vector up to four entries and a hash map beyond. Lookup walks `parent` for eight properties or fewer and otherwise uses a table built on the first lookup. Transition shapes are immutable and shared: reconfiguring a property or changing the prototype replays the object's keys from another root, so the slots keep their positions. An accessor takes two slots (getter, then setter).

Dictionary mode gives an object an unshared shape that owns a hash table and an insertion-ordered entry list, with deleted slots reused. An object enters it on deletion, past 64 properties, and the global object starts in it.

Validity cells guard prototype chains:

- An object that becomes a prototype is flagged `USED_AS_PROTOTYPE` and gets a `PrototypeInfo`: a lazily created validity cell and the list of prototype objects that inherit from it.
- Any change to a prototype object (a property added, deleted or reflagged, its `[[Prototype]]` changed) clears its own cell and, through the dependents lists, every cell downstream. The next request for the chain's cell allocates a fresh one.
- A shape caches its prototype's cell. Shapes without a prototype use the always-valid static cell 0.

The inline caches (`vm/ic.rs`, the `IcStore` inside the heap) use these as their guards. A `GetField` entry records the receiver's shape, the holder, the slot and the cell: an own hit is one shape compare and a load; a prototype hit adds the test of one boolean, with no chain walk. A `SetField` transition hit requires the shape, a valid cell, room in the slot range, and an extensible object that is not a prototype. A dictionary-mode receiver is never cached (its shape does not change with its properties); a dictionary-mode prototype is, because any change to it clears its cell. Global accesses are guarded instead by the heap's `global_generation`, bumped by every insert, remove and reflag of a global property.

Indices are recycled, so a cache entry that names a dead shape, object or cell would hit on a recycled record. Cache entries are therefore weak: every collection prunes the entries whose records were not marked before it sweeps.

## The collector

Non-moving mark-sweep (`heap/gc.rs`). Marking uses an explicit stack, never recursion; mark bits are the arenas' side bitmaps; sweep is eager and scans every arena. A collection runs only at an allocation safepoint or on an explicit `Heap::collect` (the embedding API's `Runtime::run_gc`).

### Roots

| Root | Content |
| --- | --- |
| the root stack | `RootStack`: values Rust code holds across an allocation; `mark()`, `push()`, `truncate(mark)` |
| handles | `Handles`: values held by the embedder, released explicitly; the embedding API's object and symbol values each own one |
| fixed roots | the realm's global object and intrinsics |
| the register file | `registers[..top]`: the running frame's window and every frame below it |
| the frame table | each frame's function object |
| the job queue | each pending job's values |
| atoms, the static cell, the cached closure and `arguments` shapes | |
| `kept`, `rejections`, `modules` | `WeakRef` targets kept until the current job ends, unhandled rejected promises while the embedder tracks them, every module record |

Registers above `top` belong to returned frames and may name records a collection has since freed. They are not roots. Each collection lowers `stale_from` to `top`, and a frame that resumes after a return or an unwind clears its registers from `stale_from` up before they become roots again (`interp::raise_top`). A suspended generator or async frame keeps its registers in its `Internal` record's slots, which are traced like any object's.

### The rooting rule

An allocating call (any `Heap` method that takes `&mut self` and returns `Result<_, HeapError>`) passes exactly one safepoint and roots its own arguments for that collection. Allocation takes `&mut Heap`, so a borrow of a record cannot survive a safepoint; an index can. Any index or heap `Value` held in a Rust local across a second allocating call must first be pushed on the root stack, held in a handle, or stored in a rooted object. [`core/README.md`](../core/README.md#the-rooting-rule) states the rule; the stress mode enforces it.

### Marking, weak references and sweep

1. **Mark** from the roots. Objects push their shape (shapes mark their parent chains and push the keys and prototypes they hold), their slots and elements, and their payload's values; a bytecode record marks its children and pushes its constants.
2. **Ephemerons.** A `WeakMap` entry's value is live once its key is. After the first drain each weak map reached is scanned once: a live key's value is pushed, an unmarked key's value waits in a map keyed by the key, and marking that key later pushes it. Draining may reach further weak maps, scanned in the next round, until the stack stays empty.
3. **Validity cells** owned by live prototypes are marked; the prototype table itself is weak.
4. **Weak processing.** Dead keys leave `WeakMap` and `WeakSet` tables, a dead `WeakRef` target becomes `undefined`, and a dead `FinalizationRegistry` target queues its cleanup callback as a job. The rope table, the prototype table and its dependents lists, the root-shape table and every shape's transitions drop their dead entries.
5. **Prune** the inline caches.
6. **Sweep** every arena, returning the dead objects' and bytecode records' value-heap ranges to their classes.

### The trigger

The safepoint charges each allocation's nominal bytes (record sizes, value-heap ranges at their class size, out-of-line bytes; shapes and validity cells are reserved for the cap but not charged) to a counter, and collects when the counter reaches the threshold. After every collection the counter resets and

```
threshold = max(min_heap, live_bytes × growth)
```

where `live_bytes` is the bytes held by live records and live ranges. The defaults are a 4 MiB minimum heap and growth 2. Under `GcPolicy::Manual` of the embedding API the threshold is unbounded: collections happen only at the cap and when the embedder calls `run_gc`.

| Setting | `RuntimeOptions` | `qjs` |
| --- | --- | --- |
| cap | `heap: HeapLimits { max_bytes }` (`usize::MAX`: none) | `--heap-cap-mib N` |
| minimum heap | `min_heap` (4 MiB) | `--min-heap-mib N` |
| growth | `growth` (2.0) | `--gc-growth F` (must exceed 1) |

### The memory cap

The cap limits `Heap::bytes()`: the bytes reserved by every arena (record storage, bitmaps, free lists, out-of-line bytes), the value heap's backing vector and flattened ropes. With a cap set every safepoint takes the slow path. If reserving what the allocation needs would cross the cap, the safepoint collects first (whatever the trigger says); if the heap would still cross it, the allocation fails with `HeapError::OutOfMemory` and allocates nothing. The loop and the natives turn that error into the realm's preallocated `%OutOfMemoryError%` intrinsic, an `InternalError: out of memory` created at realm initialization, so throwing it allocates nothing. The error is catchable.

Reserved memory never shrinks, and arenas and the value heap grow by doubling, so a heap stops one doubling short of the cap: a churn ring under a 32 MB cap settles at 18.8 MiB reserved. [Planned work](planned.md#3-the-heap-cap-as-a-policy) describes the cap-aware trigger and the growth step clamped to the cap.

### Stress mode

The `gc-stress` cargo feature (on `oxide-core`, forwarded by the root crate and by `quickjs-oxide-cli`) builds the stress collector. Under it:

- a safepoint collects before every N-th allocating call (`stress_interval`, 1 by default: every call);
- freed indices are quarantined for one collection, so a stale index is not recycled into a live record before the liveness assertion can catch it;
- every arena access asserts liveness, so a missed root panics with `stale … index` at the access that used it;
- after each collection `Heap::verify` re-marks from the roots and checks that live and reachable records agree, that no free-list entry is live or marked, and that every value-heap range has exactly one owner.

A heap built with the feature starts with stress on at interval 1. The embedding API's `Runtime::set_gc_stress(Some(n))` sets the interval and `None` turns stress off; `Runtime::GC_STRESS_AVAILABLE` says whether the build has it. `qjs --gc-stress=N` (`--gc-stress` alone is 1) needs a `qjs` built with the feature; without the option a stress build runs without stress. CI runs the core's tests, the lowering's tests, the two-core corpus and the fixed probes under stress, and the suite programs on a release `qjs`:

```sh
cargo test -p oxide-core --features gc-stress
cargo build -p quickjs-oxide-cli --features gc-stress --bin qjs
python3 scripts/checks/check-new-core-stress.py --qjs target/debug/qjs
```

## Statistics and the memory report

`GcStats` counts collections, total and maximum pause, records and bytes freed, and the live bytes after the last collection. `qjs --gc-stats` records every pause and prints one line at exit:

```
gc: 2 collections, total pause 2.7 ms, p50 1.846 ms, p99 1.846 ms, max 1.846 ms, live 8323 KiB, reserved 37266 KiB
```

`MemoryReport` (`heap/report.rs`; `Runtime::memory_report` in the core and the embedding API, `qjs --memory-report` at exit) reads what every part of the runtime holds from the bitmaps, free lists and capacities, without a collection:

- per arena: live records, slots, bytes used by live records, bytes reserved;
- the value heap: slots in use, free ranges by size class, capacity, bytes reserved;
- rope bytes and the atom count;
- the code store: units, instructions, position tables, source text, regular-expression programs (outside the heap);
- the register file's capacity and high-water mark, and the inline-cache entry count;
- the collector's statistics, the bytes allocated since the last collection, the threshold and the cap.

Its `reserved_bytes()` is by construction `Heap::bytes()`, the figure the cap compares against. Peak RSS also includes the process floor and the allocator's overhead, which the heap does not see; the [profiling guide](profiling.md) covers both.
