# The heap and the collector

This page describes the heap of `oxide-core` (`core/src/heap/`, `core/src/shape.rs`, `core/src/value.rs`): how values are represented, where records live, how shapes and validity cells keep inline caches valid, and how the mark-sweep collector finds its roots, when it runs, and what the memory cap does. [`core/README.md`](../core/README.md) is the core's design record and states the rooting rule in full. [Planned work](planned.md) extends this design with growth that stops at the cap and heap snapshots; [the memory plan](memory-plan.md) is the plan for peak memory against QuickJS.

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

`Arena<T>` (`heap/arena.rs`) is a `Vec<T>` with a side live bitmap and a side mark bitmap. A freed slot is overwritten with the type's `vacant()` poison record and its live bit cleared. Allocation takes the lowest clear bit of the live bitmap at or after a cursor (the lowest word a sweep or a free has touched since), testing 64 slots a word, so the lowest free index is reused first and reuse is deterministic; a free slot costs no list entry. Debug and `gc-stress` builds assert on every access that the index is live; release builds check nothing.

Arenas and the value heap grow through `reserve_exact`: a vector doubles while it is under 32 MiB, where the allocator keeps it in its heaps and every growth copies it, and grows by 2 MiB above that, where each block is its own mapping that `mremap` extends in place, so slack stays under one step. After a sweep, an arena whose trailing free run is more than a quarter of its capacity truncates its records to the last live one and gives back the capacity past one growth step.

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

A record's out-of-line bytes (string contents, BigInt limbs, internal slot vectors) are reported through `Record::extra_bytes` and count toward the heap's size. The code store (the static code units, their position tables and source text) lives beside the heap in `Runtime` for the runtime's lifetime and does not count toward the cap; the dynamic units and the regular-expression programs are in the heap ([Code compiled at run time](#code-compiled-at-run-time)).

## The value heap

`ValueHeap` (`heap/values.rs`) is one `Vec<Value>` handed out in ranges. Every property value, dense element, scope variable, constant table and child list is a range. The size classes are the powers of two and three times the powers of two (1, 2, 3, 4, 6, 8, 12, 16, … up to 2³¹), and each keeps its own free list. Named-property slots take powers of two; elements, constant tables and child lists take the nearest class, so a ten-element literal holds 12 slots. A freed range is filled with `Value::EMPTY`. Growing a range allocates a larger class, copies and frees the old one; an array past 1,024 elements grows to the next class (1.5× or 4/3×). Ranges never move.

A request first pops its class's list, then takes the next slots of the current free run (address order), then splits the smallest larger free range, and only then extends the vector. A collection's sweep records the ranges it frees in a bitmap of free slots rather than on the lists, and rebuilds the free space from the bitmap's maximal runs: a run shorter than 64 slots goes on the class lists, cut from its start into the largest classes that fit (a run of seven becomes ranges of six and one), and a longer one becomes a free run. A free run that ends the vector and spans more than a quarter of its capacity is cut off, and the capacity past one growth step is given back.

Because a range's length is always its class size, an object header stores each range as a start and a `u8` code: the class's exponent, with bit 7 set for three times a power of two. Slots, always a power of two, decode with one shift.

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

- A `StringRecord` is flat or a rope. The flat representation is canonical: `OneByte(Box<[u8]>)` whenever every code unit fits in a byte, otherwise `TwoByte(Box<[u16]>)`, so equal contents have equal representations. A rope is a concatenation whose halves live in `Heap.ropes`, keyed by the record; it flattens on the first use of its units, and its entry is dropped at the next collection after it flattens or dies. The content hash is computed on first use. A chain `'k' + x + 'v'` that the lowering turns into one `ConcatN` joins its strings once: a result under 64 units is one flat string, a longer sum ending in short pieces one rope over its first operand.
- An atom is a string record with `is_atom` set. The atom table maps a content hash to an atom index (collisions in a side list) and compares by content against the string arena, so an entry is 8 bytes and holds no second copy of the string. An atom lives as long as something holds it, like any string: a shape or dictionary key, a value, a live dynamic unit's key list ([Code compiled at run time](#code-compiled-at-run-time)); a swept atom leaves the table, and its index is reused. The atoms interned while the runtime is created (the realm's names, the builtins' properties) and the keys of the static units are pinned: `Heap.pinned_atoms` roots them for the heap's lifetime. `Heap::intern_pinned` pins a name a native spells as a literal and holds across allocations.
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

## Code compiled at run time

The units published before the program runs (the script, its modules, whatever the embedder compiles through `Runtime::compiler()`) live in the `CodeStore` beside the heap for the runtime's lifetime, and their atoms are pinned. A unit compiled at run time (`eval`, the `Function` constructors, `$262.evalScript`, a module loaded while the program runs, and what the embedder compiles through `Runtime::transient_compiler()`, the embedding API's `EvalOptions::transient` and `CompileOptions::transient`, such as a REPL's lines) lives in the heap (`heap/code.rs`, `DynamicUnits`) and is freed by the collection that finds no bytecode record naming it.

- **Reachability.** Every bytecode record names its unit. A function reaches its record; a running frame reaches its function; a suspended generator or async frame reaches its function through its internal slots; a pending job reaches the functions it calls. A unit with no live record is unreachable by any of them.
- **The loop's hold.** The loop holds the running unit through an `Rc`, so a collection inside a call never frees the instructions it is executing; by the time a unit is swept no frame names it and its count is one, which debug builds assert. A static unit is borrowed with no count.
- **A compilation's window.** Between `publish` and `instantiate` a unit has no record. A `Compiler` pins the units it publishes and holds the atoms it interns (keys, names, string constants), each once, until it is dropped; a `Compiler::reborrow` (a host callback's view) ends nothing. A compilation that fails leaves its units to the next collection.
- **Atoms.** Publishing a unit records the distinct atoms its instructions name as keys, and its name (`CodeUnit::atoms`); a live unit marks them.
- **Cache ranges.** A unit's cache entries are one range of the `IcStore`. A freed unit's range is emptied and returned to a free list sorted by position, where adjacent ranges merge; the next unit takes the first range that fits. Polymorphic rows are reused the same way.
- **Regular expressions.** A program is compiled once per distinct `(source, flags)` and named by the payload of every `RegExp` object over it; a collection frees the programs no live object names, and their keys leave the index.

Under `gc-stress`, freed unit slots and program slots are quarantined for one collection like arena indices, and `Heap::verify` checks that exactly the reachable units and programs are live and that every free cache range is empty.

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
| pinned and held atoms, pinned dynamic units, the static cell, the cached closure and `arguments` shapes | atoms that live as long as the heap; the atoms and the published units of a compilation still running |
| `kept`, `rejections`, `modules` | `WeakRef` targets kept until the current job ends, unhandled rejected promises while the embedder tracks them, every module record |

Registers above `top` belong to returned frames and may name records a collection has since freed. They are not roots. Each collection lowers `stale_from` to `top`, and a frame that resumes after a return or an unwind clears its registers from `stale_from` up before they become roots again (`interp::raise_top`). A suspended generator or async frame keeps its registers in its `Internal` record's slots, which are traced like any object's.

### The rooting rule

An allocating call (any `Heap` method that takes `&mut self` and returns `Result<_, HeapError>`) passes exactly one safepoint and roots its own arguments for that collection. Allocation takes `&mut Heap`, so a borrow of a record cannot survive a safepoint; an index can. Any index or heap `Value` held in a Rust local across a second allocating call must first be pushed on the root stack, held in a handle, or stored in a rooted object. [`core/README.md`](../core/README.md#the-rooting-rule) states the rule; the stress mode enforces it.

### Marking, weak references and sweep

1. **Mark** from the roots. Objects push their shape (shapes mark their parent chains and push the keys and prototypes they hold), their slots and elements, and their payload's values; a bytecode record marks its children, pushes its constants and marks its unit when it is dynamic, pushing the unit's atoms; a `RegExp` object marks its program and pushes the program's source and flags atoms; a module namespace pushes its export keys.
2. **Ephemerons.** A `WeakMap` entry's value is live once its key is. After the first drain each weak map reached is scanned once: a live key's value is pushed, an unmarked key's value waits in a map keyed by the key, and marking that key later pushes it. Draining may reach further weak maps, scanned in the next round, until the stack stays empty.
3. **Validity cells** owned by live prototypes are marked; the prototype table itself is weak.
4. **Weak processing.** Dead keys leave `WeakMap` and `WeakSet` tables, a dead `WeakRef` target becomes `undefined`, and a dead `FinalizationRegistry` target queues its cleanup callback as a job. The rope table, the prototype table and its dependents lists, the root-shape table and every shape's transitions drop their dead entries.
5. **Prune** the inline caches, including the megamorphic entries whose key atom dies.
6. **Sweep** every arena, recording the dead objects' and bytecode records' value-heap ranges in the value heap's bitmap of free slots, then rebuild the value heap's free space and shrink what has a long free end; a dead atom leaves the atom table. Unmarked dynamic units and regular-expression programs are freed, and a freed unit's cache range returns to the `IcStore`.

### The trigger

The trigger bounds `Heap::bytes()`, the bytes reserved by the arenas, the value heap, the tables beside them, the live dynamic units and the regular-expression programs, by a budget. After every collection that leaves `live_bytes` (the bytes held by live records and live ranges, the live dynamic units and the regular-expression programs),

```
budget = max(min_heap, live_bytes × (1 + growth))
threshold = budget − live_bytes
```

The safepoint charges each allocation's nominal bytes (record sizes, value-heap ranges at their class size, out-of-line bytes; shapes and validity cells are reserved for the cap but not charged) to a counter and collects when the counter reaches the threshold, so the bytes in use stay within the budget (publishing a dynamic unit charges its bytes to the same counter), and so does reserved memory while the free space matches the requests. Every growth step of an arena or the value heap is charged to the room the budget leaves above reserved memory; a step past it zeroes the threshold, so the next safepoint collects. A collection that leaves reserved memory above the budget raises the budget to one step (a quarter of the heap, between 64 KiB and 2 MiB) above it, so the heap grows by at most one step between collections. Under a cap the budget is at most one step below the cap.

The defaults are a 1 MiB minimum heap and growth 2. Under `GcPolicy::Manual` of the embedding API the threshold and the room are unbounded: collections happen only at the cap and when the embedder calls `run_gc`.

| Setting | `RuntimeOptions` | `qjs` |
| --- | --- | --- |
| cap | `heap: HeapLimits { max_bytes }` (`usize::MAX`: none) | `--heap-cap-mib N` |
| minimum heap | `min_heap` (1 MiB) | `--min-heap-mib N` |
| growth | `growth` (2.0) | `--gc-growth F` (must exceed 1) |

### The memory cap

The cap limits `Heap::bytes()`: the bytes reserved by every arena (record storage, bitmaps, out-of-line bytes), the value heap (its backing vector, class lists and bitmap), flattened ropes, the ropes' halves table, the atom table, the inline caches, the live dynamic units and the regular-expression programs. Publishing a unit does not check the cap; the next safepoint collects or fails with `OutOfMemory` if the heap is over it. With a cap set every safepoint takes the slow path. If reserving what the allocation needs would cross the cap, the safepoint collects first (whatever the trigger says); if the heap would still cross it, the allocation fails with `HeapError::OutOfMemory` and allocates nothing. The loop and the natives turn that error into the realm's preallocated `%OutOfMemoryError%` intrinsic, an `InternalError: out of memory` created at realm initialization, so throwing it allocates nothing. The error is catchable.

Under a cap the budget stops one growth step short of it: the churn ring of `core/examples/workloads.rs` (`churn 3000000 --ring 65536 --cap-mib 32`) settles at 18.9 MiB reserved. [Planned work](planned.md#3-the-heap-cap-as-a-policy) describes the growth step clamped to the cap.

### Stress mode

The `gc-stress` cargo feature (on `oxide-core`, forwarded by the root crate and by `quickjs-oxide-cli`) builds the stress collector. Under it:

- a safepoint collects before every N-th allocating call (`stress_interval`, 1 by default: every call);
- arena slots freed by a sweep are quarantined in a second bitmap until the next sweep, and a shrink keeps them, so a stale index is not recycled into a live record before the liveness assertion can catch it;
- every arena access asserts liveness, so a missed root panics with `stale … index` at the access that used it;
- after each collection `Heap::verify` re-marks from the roots and checks that live and reachable records agree, that no free slot is marked and every arena's free count and cursor agree with its bitmap, and that every value-heap slot is owned by exactly one live range or free range or run.

A heap built with the feature starts with stress on at interval 1. The embedding API's `Runtime::set_gc_stress(Some(n))` sets the interval and `None` turns stress off; `Runtime::GC_STRESS_AVAILABLE` says whether the build has it. `qjs --gc-stress=N` (`--gc-stress` alone is 1) needs a `qjs` built with the feature; without the option a stress build runs without stress. CI runs the core's tests, the lowering's tests, the two-core corpus and the fixed probes under stress, and the suite programs on a release `qjs`:

```sh
cargo test -p oxide-core --features gc-stress
cargo build -p quickjs-oxide-cli --features gc-stress --bin qjs
python3 scripts/checks/check-gc-stress.py --qjs target/debug/qjs
```

## Statistics and the memory report

`GcStats` counts collections, total and maximum pause, records and bytes freed, and the live bytes after the last collection. `qjs --gc-stats` records every pause and prints one line at exit:

```
gc: 2 collections, total pause 2.7 ms, p50 1.846 ms, p99 1.846 ms, max 1.846 ms, live 8323 KiB, reserved 37266 KiB
```

`MemoryReport` (`heap/report.rs`; `Runtime::memory_report` in the core and the embedding API, `qjs --memory-report` at exit) reads what every part of the runtime holds from the bitmaps, the value heap's free lists and runs and the capacities, without a collection:

- per arena: live records, slots, bytes used by live records, bytes reserved;
- the value heap: slots in use, free ranges by size class and free runs, capacity, bytes reserved;
- rope bytes, the atom count and the pinned atoms among them, and the bytes of the tables beside the arenas;
- the code: units (static and dynamic counted apart), instructions, position tables, source text, the bytes the dynamic units hold (counted in `reserved_bytes()`), regular-expression programs;
- the register file's capacity and high-water mark, and the inline-cache entries, held and free;
- the collector's statistics, the bytes allocated since the last collection, the threshold, the budget and the cap.

Its `reserved_bytes()` is by construction `Heap::bytes()`, the figure the cap compares against. Peak RSS also includes the process floor and the allocator's overhead, which the heap does not see; the [profiling guide](profiling.md) covers both.
