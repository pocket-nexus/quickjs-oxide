# Planned work

This page is the plan for the memory and embedding-target work on `oxide-core` tracked in issue [#122](https://github.com/pocket-nexus/quickjs-oxide/issues/122) ("optional copying nursery; heap cap policy; position-independent heap snapshots and bytecode precompilation (indices need no pointer rewriting); 32-bit MIPS build", exit: "controlled workloads under PSP-class memory; snapshot load time"). It is written for whoever implements an item: it fixes the decisions that are made, lists the open ones with the item that decides each, and gives every item an acceptance check that can be run without judgment calls. Day estimates order the work; they are not commitments.

The work is a list of items taken by need, in the priority order of section 8; none has a consumer yet, and each can be picked up from this page without re-planning. The items build on the current implementation: the heap, the collector, its trigger and the cap described in [Heap](heap.md); the loop, frames and caches in [Execution](performance/execution.md); the rooting rule and the core's design record in [`core/README.md`](../core/README.md); the crates and the embedding API in [Architecture](architecture.md). Measurements follow [Measurement method](performance/measurement.md).

## 0. Scope

**In scope.** The heap cap as a policy rather than a check (the trigger under a cap, growth that stops at the cap, giving memory back, the out-of-memory error); reclaiming the code of dead dynamically compiled functions; runtime options for small targets (heap, register file, native stack); code images (lowered units serialized without heap state, loaded into any runtime of the same build) and heap snapshots (the whole runtime at a quiescent point, loaded in place of initialization); 32-bit correctness and memory on a tier-2 32-bit target; `oxide-core` without the standard library, for the console ports; the measurements of section 9. The generational collector of section 6 is an optional, measurement-gated item.

**Outside this plan.** The shape of the embedding API beyond the entry points named in 4.4 and the `Host::math` method of 5.4; a JIT; linking for the PSP itself, whose Rust target is tier 3 and needs a nightly toolchain with `-Zbuild-std` (section 5.3 says what this plan delivers instead and how a port consumes it); any change to the loop-size gate or the instruction gate of [Execution principles](performance/principles.md).

**Fixed constraints.** The collector's trigger keeps its 4 MiB minimum heap and growth 2; the generational collector of section 6 is an addition behind that trigger, and the maintainer may strike section 6 before it starts.

**What motivates the work.** The core is at or below QuickJS's time on most suite programs; its memory is not. Measured on the reference machine at the default trigger, one `run()` per program:

| Program | Peak RSS QuickJS / `oxide-core` | Heap reserved | Live at exit | Collector share |
| --- | --- | ---: | ---: | ---: |
| Splay (both) | 144 / 265 MiB | 293.6 MiB | 75.0 MiB | 33–34% |
| Code-Load | 19 / 121 MiB | 5.9 MiB | 1.4 MiB | 0.7% |
| Mandreel | 57 / 100 MiB | 1.4 MiB | 1.0 MiB | 0.0% |
| PdfJS | 22 / 68 MiB | 43.2 MiB | 5.6 MiB | 6.8% |
| TypeScript | 98 / 90 MiB | 95.7 MiB | 20.3 MiB | 0.9% |
| Havlak | 33 / 41 MiB | 49.6 MiB | 8.3 MiB | 11.1% |
| zlib | 134 / 20 MiB | 1.4 MiB | 356 KiB | 0.0% |

Three mechanisms explain the table, and each has a section below:

- **Reserved memory never comes back.** Arenas and the value heap are `Vec`s that grow by doubling and never shrink; Splay reserves 294 MiB for 75 MiB live, Havlak 50 for 8. Under a cap this is the difference between fitting and `OutOfMemory` (section 3).
- **Code lives as long as the runtime.** Code-Load's heap is 6 MiB and its process 121 MiB: the units `eval` and `Function` publish, their source text and their position tables are kept until the runtime is dropped. Mandreel's 100 MiB is the units of its multi-megabyte source plus the buffers the program keeps; its heap is 1.4 MiB (section 3.3).
- **Every collection marks the whole live set.** Splay spends a third of its time in nine collections of 86–115 ms each over a 75 MiB live set (section 6).

An empty script costs about 3.1 million user-space instructions on the release `qjs`, and 16 MiB of RSS, which is the process floor. The realm's initialization is what a heap snapshot stands in for (section 4).

## 1. Location

| Module | Crate | Content |
| --- | --- | --- |
| `heap::gc` | `oxide-core` | the cap-aware trigger, growth that stops at the cap, shrinking after a collection (3.1, 3.2); the generational option (6) |
| `heap::report` | `oxide-core` | `MemoryReport`, with the fields section 2 adds |
| `bytecode` | `oxide-core` | reclaiming dynamic units (3.3); the code image's unit encoding (4.2) |
| `snapshot` (new) | `oxide-core` | the snapshot writer and loader over every arena and store (4.3) |
| `runtime` | `oxide-core` | the behavior behind `RuntimeOptions` (3.4); `Runtime::snapshot`, `Runtime::from_snapshot`, `Runtime::load_image` (4.4) |
| `host` | `oxide-core` | the `std` feature split: streams, clock and file access as `std`-only defaults; the math provider (5.4) |
| `heap::hash`, `shape`, `heap::atom`, … | `oxide-core` | the core's own hash table in place of `std::collections::HashMap` (5.4) |
| `api` | root crate | the image encoding of `CompiledUnits` |
| `apps/cli` (`qjs`) | CLI | `--compile`, `--snapshot`, `--load-snapshot` |
| `scripts/benchmark`, `docs/performance/probes` | tools | the runners of the section 9 tables |
| `.github/workflows/ci.yml` | CI | the `i686` and `no_std` checks in `fast`; a `suite-32bit` tier (section 7) |

Delivery: one pull request per item, in the order of section 8, each with its acceptance check in the description and the gates of the [pull request template](../.github/pull_request_template.md): `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `cargo test --locked --workspace --all-targets`, `cargo test -p oxide-core --features gc-stress`, `core/tests/corpus` under `gc-stress`, `scripts/checks/check-gc-stress.py`, the loop-size check (`vm::interp::run_at` under 16,384 bytes; it is 15,226), and the instruction gate (`instructions:u` of the eight V8 v7 programs against the base branch's `qjs`, a rise above 2% explained by a profile or fixed before merge). The Test262 focused vector replays in `fast`, and an item that changes an observable behavior (3.4's error, 5.4's math) re-freezes the full receipt.

## 2. Memory accounting

`Runtime::memory_report() -> MemoryReport` is the one source of every memory number these items report and the embedder reads. It walks the arena bitmaps, the free lists and the capacities without a collection, and reports per arena, the value heap, ropes, atoms, the code store, the regular-expression programs, the register file, the inline caches and the collector ([Heap](heap.md#statistics-and-the-memory-report) lists the fields). Its `reserved_bytes()` is by construction `Heap::bytes()`, the figure the cap compares against.

The items add to it what they need to report:

| Field | Content | For |
| --- | --- | --- |
| code store | static and dynamic units counted separately, handler and capture bytes | 3.3 |
| atoms | the table's capacity beside the count | 3.2 |
| per arena | bytes the records own out of line (string units, BigInt limbs, buffers, tables, suspended frames), beside bytes reserved | 3.2 |

`qjs --memory-report` prints it at exit, and `fixed.py` reads peak RSS from the operating system; the two are reported side by side, since the process floor (16 MiB on an empty script) and the allocator's own overhead are outside the heap's view.

**Acceptance (heap cap item, first step).** The report of each of the 37 suite programs at one `run()` is recorded as the first column of the memory table (section 9) before anything else changes.

## 3. The heap cap as a policy

### 3.1 The trigger under a cap

The safepoint compares the bytes allocated since the last collection against `threshold = max(min_heap, live × growth)` and, when a cap is set, also checks whether the growth the allocation needs would cross the cap, collecting or failing ([Heap](heap.md#the-trigger)). Growth 2 over a 75 MiB live set puts the next collection at 150 MiB whatever the cap is, so under a 128 MiB cap every Splay allocation past 128 MiB is a collection (the cap check fires on each one once the heap sits at the cap). The planned rule:

```text
threshold = clamp(live × growth, min_heap, cap − headroom)      when a cap is set
headroom  = the largest single growth step any arena or the value heap would take next
```

so that under a cap the collection happens before the heap reaches it, and the heap sits below the cap by one growth step rather than at it. `growth` stays 2 and `min_heap` 4 MiB; only the clamp is new, and it is inactive without a cap, so uncapped runs are unchanged by construction (the instruction gate confirms it).

### 3.2 Growth that stops at the cap, and shrinking

- **Growth.** An arena or value-heap growth step that would cross the cap grows to the cap instead of doubling. `Arena::growth_bytes` and `ValueHeap::growth_bytes` compute the step; it is clamped where it is taken.
- **Shrinking after a collection.** After a sweep, every arena whose trailing free run is more than a quarter of its capacity and whose free share is above half gives the trailing run back (`Vec::truncate` plus `shrink_to`; the free list drops the indices above the new length). The value heap does the same with free ranges that end at its high-water mark; a non-moving heap can give back only what is free at the top, and the item records what that recovers on Splay, Havlak, PdfJS and TypeScript. Shrinking runs only when `bytes > 2 × live` after the collection, so the steady state of a program that fills and refills the same heap (the churn ring of `docs/performance/probes/slopes/heap_workloads.py`) pays nothing.
- **The register file** grows by doubling up to `RuntimeOptions.stack_limit`; it shrinks back to its initial reservation at the runtime boundary when the frame table is empty and it is above four times its initial size.

### 3.3 Reclaiming dynamic units

A unit published at run time (`eval`, the `Function` constructors, dynamic `import()` of a new module) lives in `CodeStore.dynamic`: write-once chunks of `OnceCell`s, published through `&self` because the loop holds the running unit's `&CodeUnit` across heap calls, and kept for the runtime's lifetime. A `FunctionBytecode` record (a heap record, traced) names its unit, so after marking the heap knows which dynamic units no live function and no running frame reaches.

Decision: **dynamic units are reclaimed at the collection that finds them dead, with one cell per unit.** `CodeStore.dynamic`'s slots become per-slot cells (`RefCell<Option<Box<CodeUnit>>>`): the loop borrows the running unit's slot for the frame's duration (one borrow-flag increment at frame entry and one decrement at return, on dynamic units only; static units keep a plain `&CodeUnit`), and a dead unit's slot is by definition unborrowed, so the sweep frees it and returns its inline-cache range to the `IcStore` for reuse (the store gains a free list of ranges beside `ic_len`). A freed slot is reused by the next dynamic publication. Static units (published before the program runs) stay for the runtime's lifetime: they are the program, and section 4's snapshot serializes them as a whole.

If the per-frame borrow costs the eight V8 v7 programs more than the 2% gate (Code-Load is the only suite program that runs dynamic units in its hot path; the eight run none, so the expected cost is the kind test at frame entry), the fallback is reclamation at the runtime boundary (`run_script`, `run_jobs`, the host's entry points), where `&mut CodeStore` is available with no loop running; the item records which one it builds and Code-Load's peak RSS under each.

**Acceptance (dynamic code item).** A program that compiles 10⁵ functions through `new Function` and keeps none has a steady `MemoryReport` code-store size and a steady peak RSS under a 32 MiB cap; a program that keeps every tenth one keeps exactly those units; a function returned from `eval` and called after the collection that freed its siblings runs (`core/tests/corpus` gains these programs); Code-Load's peak RSS is within 2× QuickJS's (19 MiB) in the memory table.

### 3.4 Runtime options and the out-of-memory error

`RuntimeOptions` (`core/src/runtime.rs`) holds the cap (`heap: HeapLimits`), `min_heap`, `growth`, `stack_limit` (registers, 2²⁰ by default), `native_stack_budget` (1 MiB) and `native_depth_limit`; `qjs` exposes them as `--heap-cap-mib`, `--min-heap-mib`, `--gc-growth` and `--stack-limit`. This item adds the behavior of 3.1 and 3.2 behind them, the register file's shrink, and one field:

```rust
pub initial_registers: u32,      // the register file's first reservation
```

A PSP-class profile is one `RuntimeOptions` value, documented in `core/README.md` (a 24 MiB cap, a 1 MiB minimum heap, a 2¹⁶-register file, a 256 KiB native budget), not a build configuration.

`HeapError::OutOfMemory` is thrown as the realm's preallocated `%OutOfMemoryError%` intrinsic (`InternalError: out of memory`, QuickJS's), which is catchable. The item makes every allocating opcode and native reach `vm::error::oom` rather than unwrap, keeps the thrown value free of allocation, and checks that the heap is usable after the catch (a failed allocation leaves the heap unchanged). A `try { … } catch` loop that fills the heap to the cap and recovers 100 times is the test, under `gc-stress`.

### 3.5 PSP-class controlled workloads

The "controlled workloads under PSP-class memory" of #122's exit are: the 37 suite programs at one `run()` and the 20 fixed probes at smoke size, under caps of 32, 24 and 16 MiB, plus the churn ring (`heap_workloads.py churn`, a 12–16 MB live set) under an 8 MiB cap once the ring is sized to 4 MiB live, on x86_64 and on the 32-bit target of section 5, each recording completion or `OutOfMemory`, collections, pauses, heap reserved and peak RSS (the capped-run table). The pass condition: under a cap of C, every program whose live set at exit is below C/2 completes with the manifest's stdout, and its peak RSS is at most C plus the process floor plus 10%. Splay (75 MiB live) is expected to fail every cap and is reported, not excused; QuickJS cannot run it in 32 MiB either.

## 4. Code images and heap snapshots

### 4.1 Two artifacts

| Artifact | Content | Loaded into | Saves |
| --- | --- | --- | --- |
| **code image** (`.oxc`) | the lowered units of one script or module graph, with their constants by value (atoms by text, numbers, BigInt digits, regular expressions as source and flags), no heap state | any runtime of the same build, by interning the constants and publishing the units | parsing, compiling and lowering the source; the front end need not be on the device |
| **heap snapshot** (`.oxs`) | every arena, the value heap, the atom table, the shapes, the realm, the code store and the regular-expression store of a runtime at a quiescent point | a new runtime, in place of `Runtime::new` and whatever setup scripts ran before the snapshot | the realm's initialization (about 3.1 M instructions) and the program's own setup |

A device port compiles on the host (`qjs --compile`) and ships images; in a build without a compile host, `eval`, the `Function` constructors and dynamic `import()` of source throw `InternalError: no compiler`, an entry the [deviation ledger](deviations.md) gains for such builds. A port that wants a warm start ships a snapshot of the realm plus its framework.

### 4.2 Position independence and the format

Every cross-record reference in the core is an index (`ObjectId`, `StringId`, `ShapeId`, …, a `Value` payload, a `CodeUnitId`, an `IcStore` site), never an address; a native function is the discriminant of the `Native` enum; a bytecode record names its unit by index ([Heap](heap.md#values)). A snapshot therefore writes each arena at its current indices and nothing is relocated at load. The format:

- little-endian; every count and index is a `u32`; every `Value` is its `u64`; `f64` as its bits; every width fixed, so the bytes are identical from a 32-bit and a 64-bit build of the same source (the 32-bit item's test);
- a header with a magic, a format version (a constant bumped by hand) and a **build fingerprint**: a hash of the `Opcode` and `Native` name tables and of the record sizes, computed once at startup, so a file from another build is rejected with `SnapshotError::Fingerprint` before any of it is read;
- per arena: the live bitmap, then the live records in index order; the loader fills dead indices into the free list from the bitmap. Record fields that own Rust allocations (string units, BigInt limbs, buffers, the shape's transitions and lazy table, the collection tables) are written as their contents and rebuilt as allocations by the loader;
- hash tables are rebuilt at load from what they index: the atom table from the records flagged `is_atom` (their stored `hash` is written, so rebuilding is insertion without rehashing), `shape_roots` from the shapes without a parent, each shape's `transitions` from its children's parent links, `prototypes` (cells and dependents) from the shapes' prototype fields and the cell arena, the regular-expression index from the records. A shape's lazy property table stays unbuilt until the first lookup that wants it;
- the inline caches start empty: the loader reserves each unit's `ic_count` entries in publication order, so `ic_base` is unchanged;
- the code store writes static and dynamic units alike (a `Function`-made function live in the heap keeps its unit), each as its `Insn`s, handlers, captures, children, flags, counts, name atom, length, the position table and, unless the snapshot is stripped, the source text; `DebugInfo.filename` goes through one string table;
- ropes are flattened by the writer (a snapshot runs a full collection first, which the writer triggers; flattening is what the next operation on the rope would do anyway);
- a `SharedArrayBuffer`'s bytes are written as bytes (sharing is a property of a running process); a detached buffer writes its flag;
- of the collector's state, only `live_bytes` is written; the loaded runtime starts with `allocated = 0` and the usual threshold rule.

A code image is the code-store section of the format plus a constant table (the strings, numbers, BigInts and regular expressions the units name) and the tree `Runtime::instantiate` takes; loading publishes through `CodeStore::publish`, so the **publication validator runs on every image**: a malformed image fails at load with the validator's error, never at run. Template-site objects, which the lowering caches in the constant table once a tagged template is evaluated, are written empty and rebuilt on first use.

### 4.3 Quiescence

`Runtime::snapshot` requires and checks: no frame on the register file, an empty job queue, an empty kept-objects list, no live handles, no agent threads, and no `Atomics.wait` waiter. Anything else is `SnapshotError::NotQuiescent(reason)`. Suspended generators and async activations are heap records and are written like any object; a pending promise is data; a `WeakRef` whose target died reads `undefined` after the load as it would after a collection. The host's state (`Host`, `Services`) stays outside the snapshot; the loader takes a fresh `Host`.

### 4.4 API and driver

```rust
impl Runtime {
    pub fn snapshot(&mut self, options: SnapshotOptions) -> Result<Vec<u8>, SnapshotError>;   // strip_source: bool
    pub fn from_snapshot(bytes: &[u8], host: Box<dyn Host>, options: RuntimeOptions) -> Result<Runtime, SnapshotError>;
    pub fn load_image(&mut self, bytes: &[u8]) -> Result<ObjectId, LoadError>;                  // the script or module function
}
```

The embedding API re-exports the three. The root crate's `api` gains `encode_image(CompiledUnits) -> Vec<u8>` beside its lowering entry, and `qjs` gains `--compile OUT file.js` (an image of one script; with `--module`, of a module graph resolved through the module loader), `--snapshot OUT [setup.js …]` (a realm, or a realm after the setup scripts, honoring `--strip-source`), `--load-snapshot FILE` (start from it instead of `Runtime::new`), and runs a `.oxc` argument through `load_image`. Images and snapshots are untrusted input to their loaders: every length is bounds-checked against the buffer, every index against the arena it names, and two fuzz-style tests (truncation at every byte offset of a realm snapshot, and a byte flip at every offset of a small image) must end in `Err`, never a panic.

### 4.5 What is measured

The snapshot and image table: for the realm alone and for the realm plus each of three setup corpora (the AWFY bundle, Octane TypeScript, Octane Mandreel: the largest sources), the snapshot's size, the instructions and time of `from_snapshot` against those of `Runtime::new` plus compiling, lowering and running the setup; for each of the 37 programs, the image's size and the instructions of `load_image` against the compile-and-lower path (Code-Load excluded, since its work is compiling at run time). The gate: the realm snapshot loads in fewer than half the instructions of the cold initialization it stands in for (below 1.5 M), and every image loads in fewer than a tenth of the instructions its compilation takes.

## 5. 32-bit targets and the standard library

### 5.1 The 32-bit gate target

The ports in view (PSP, Vita, 3DS, ESP-IDF) are MIPS and ARM 32-bit targets without a hosted operating system. The pinned stable toolchain's `rustup` offers no MIPS target (they are tier 3), and `mipsel-sony-psp` needs a nightly compiler with `-Zbuild-std`. The 32-bit gate therefore runs on **`i686-unknown-linux-musl`**: tier 2, statically linked with the toolchain's own C runtime, built and executed on the x86_64 reference machine and on CI's runners without an emulator. What it proves is what the ports need from this repository: `Value` is 8 bytes and `Insn` 8 bytes there as here, indices are `u32`, every `usize` that reaches a format or a limit is deliberate, and the whole engine (core and front end) runs the corpus, the probes, the 37 programs and the focused Test262 vector on a 32-bit word. `wasm32-unknown-unknown`, a release target through `apps/web`, is the second 32-bit build the `fast` job checks.

**Acceptance (32-bit item).** `cargo test -p oxide-core --target i686-unknown-linux-musl` with and without `gc-stress`; `qjs` built for the target runs `core/tests/corpus` and the 20 probes at smoke size under `--gc-stress=1` and the 37 programs at one `run()` under the `suite-stress` intervals; `run-test262` built for the target replays the frozen focused vector; the realm snapshot and the AWFY image written by the i686 binary and by the x86_64 binary are byte-identical, and each loads the other's; the 32-bit table records the eight programs' instructions and time on i686 against x86_64 (the instruction gate is an x86_64 measurement and is not applied to i686).

### 5.2 The audit

The 32-bit item includes a one-time audit, recorded in its pull request: every `usize` in `oxide-core`'s public types and in anything written to a file is justified or changed to `u32`/`u64`; every `as usize` from a `u64` is on a value known to fit; `size_of` assertions for `Value`, `Insn`, `Frame`, `ObjectHeader`, `StringRecord` and `Shape` are tests that run on both widths and record both numbers in `core/README.md`; `AtomicU64` (used by `Atomics` on 64-bit element kinds) is available on i686 and on the ports' cores, and the agents' `Condvar` path is `std`-only (5.4).

### 5.3 What the ports get

This plan stops short of linking for a console. It makes `oxide-core` buildable without the standard library (5.4), verified on a tier-2 target with no `std` at all (`thumbv7em-none-eabi`, core and alloc only, installed by `rustup` on the pinned stable toolchain), and documents in `core/README.md` the recipe a port follows: a nightly toolchain, `-Zbuild-std=core,alloc`, the target's JSON or tier-3 name, a `Host` implementation over the SDK (streams, clock, time zone, file access, the math provider), `RuntimeOptions` for the device, and code images compiled on the host. Whether this repository also runs that recipe in a non-gating weekly job with a nightly toolchain is the maintainer's call (section 10).

### 5.4 `oxide-core` without `std`

`oxide-core` gets a default feature `std`; without it the crate is `#![no_std]` with `alloc`. What `std` provides and what stands in for it:

| Use | With `std` | Without `std` |
| --- | --- | --- |
| `Vec`, `Box`, `String`, `Rc`, `VecDeque`, `BTreeMap` | `std` | `alloc`, the same types |
| `std::collections::HashMap` with the core's `FxHasher` (shapes, transitions, the atom table, `shape_roots`, `prototypes`, ropes, ephemerons, the regular-expression index, two in the VM) | `std` | the core's own open-addressing table, generalized from `heap::table::Table` (the index of the `Map` and `Set` storage), with the same `FxHasher`, used by both builds so the `std` build is measured by the instruction gate (none of these tables is on a cache hit path) |
| `Instant` for pause timing | `std` | pauses untimed; `GcStats` keeps counts and bytes |
| `Host` defaults: process streams, `SystemTime`, `read_to_string` | `std` | the port implements `write`, `now_ms`, `local_tz_offset_minutes`; `read_file` defaults to `None` |
| threads, `Mutex`, `Condvar`, `mpsc` (the Test262 agents, a blocking `Atomics.wait`) | `std` | the `agent` module is `std`-only; `Atomics.wait` behaves as with `CanBlockIsFalse` (`TypeError`); `Atomics.notify` returns 0; `SharedArrayBuffer` keeps `Arc<[AtomicU8]>` (in `alloc` and `core`) |
| `std::error::Error` impls | `std` | `core::error::Error` |
| `f64` transcendental and rounding methods (`sin`, `exp`, `ln`, `pow`, `sqrt`, `floor`, `round`, …; `Math`, `Date`, radix formatting, `Number.prototype.toFixed` and friends) | the exactly specified ones (`floor`, `ceil`, `trunc`, `round`, `fmod` via `%`, `sqrt` correctly rounded) are the core's own bit-exact implementations in both builds; the transcendental ones go through `Host::math(op, x, y)`, whose `std` default calls the standard library | `Host::math` is the port's SDK libm, which is what QuickJS on that platform computes too |

The core keeps zero dependencies beyond `oxide-regexp`: its own hash table, its own exact rounding functions. `oxide-regexp` gets the same `std` feature if it needs one (it is a table-driven matcher over `u16` slices; the item checks).

**Acceptance (`no_std` item).** `cargo check -p oxide-core --no-default-features --target thumbv7em-none-eabi` in `fast`; `cargo test -p oxide-core --no-default-features` on x86_64 (the tests that need threads or timing are `std`-gated); the instruction gate on the eight with the core's own hash table; the full Test262 receipt re-frozen if any `Math` or `Date` result changes (none should: the `std` default is the same library call).

## 6. The generational collector (optional)

#122 lists "an optional copying nursery, decided on measurements". The measurement: Splay spends 33% of its time in nine full collections over a 75 MiB live set, Havlak 11%, every other suite program under 8%. The lever is worth 20–25% of Splay's time, and little elsewhere.

The mechanism that fits a non-moving arena heap is **generational marking with sticky mark bits**, which keeps every record at its index: moving a young object would mean renumbering it, and its index is held in the register file, the root stack, the handles, every object slot and element, the `IcStore`'s entries, the `shape_roots` and `prototypes` keys and the frame table, and section 4 relies on indices staying fixed. After a full collection the mark bits of survivors stay set and mean *old*; a minor collection marks from the roots and from the remembered set, stops at every marked object, and sweeps only the records allocated since the last collection (an allocation log per arena). What it needs:

- a **write barrier** on every store of a heap value into an object's slot, element, cell or table: `if marked(object) && value.is_heap() && !marked(value) { remember(object) }`, which on the `SetField` hit path is two bit loads and two compares. The stores: the `SetField` and `SetElem` hit and slow paths, `StoreCell`/`StoreCaptured`, `Heap::set_slot`, dictionary and collection-table writes, element pushes, the natives' writes through `property::set` and `define_own`. A missed one is a dangling index the stress mode finds, so the barrier lives in the few `Heap` entry points that write a value into a record, not at call sites;
- the **remembered set** (a `Vec<u32>` per arena, with a per-record flag so an object is remembered once), the **allocation log**, a per-arena *young* count feeding a minor trigger (`young_bytes ≥ young_threshold`, 1 MiB to start), and the full trigger unchanged (growth 2 over `live`);
- the stress mode and the verifier extended: under `gc-stress` every N-th allocation runs a minor collection and every M-th a full one, and the verifier checks after a minor collection that no old object points to an unmarked young one without being remembered.

**Go/no-go, recorded in the item's pull request.** Built behind a Cargo feature so the default binary is unchanged until the decision. Adopted (the feature becomes the default and is removed as a feature) only if all three hold: Splay's collector share drops below 15% and its time by at least 15%; no V8 v7 program retires more than 2% more instructions than the base branch's binary (the barrier's cost); every other program's time is within ±3%, the layout noise of [Execution principles](performance/principles.md#measurement). Otherwise the code is removed in the same pull request, and the pull request records the numbers. The maintainer may strike this section before it starts; sections 2–5 and their exit conditions do not depend on it.

## 7. The driver and CI

`qjs` grows `--compile`, `--snapshot`, `--load-snapshot`, honors `--strip-source` for both, and accepts `.oxc` files as programs. The churn ring of `heap_workloads.py` gains the cap and the report.

CI:
- `fast`: `cargo check -p oxide-core --no-default-features --target thumbv7em-none-eabi` (`no_std` item); `cargo check --target i686-unknown-linux-musl -p quickjs-oxide-cli` (32-bit item); the snapshot round-trip, determinism and truncation tests run inside `cargo test -p oxide-core` (snapshot item).
- `suite-32bit` (nightly and dispatch, with the 32-bit item): build the i686 `qjs` and `run-test262` in release, run the corpus and the probes under stress, the 37 programs at one `run()` under the `suite-stress` intervals, and the frozen focused vector; compare the realm snapshot with the x86_64 build's byte for byte.
- `suite-stress` additionally runs the 37 programs at one `run()` under a 32 MiB cap (heap cap item) and the churn ring under 8 MiB, checking the pass condition of 3.5.
- Reference-machine measurements (the instruction gate, the section 9 tables) are recorded in pull requests, not run in CI.

## 8. Items and order

| Item | Content | Acceptance | Taken when | Estimate |
| --- | --- | --- | --- | --- |
| Heap cap | the memory table's first column; the cap-aware trigger, growth to the cap, shrinking, the register file's limit and shrink, the catchable `OutOfMemory` (3.1, 3.2, 3.4); the suite under a 32 MiB cap in `suite-stress` | the 100-recoveries test under stress; the shrink numbers for Splay, Havlak, PdfJS, TypeScript recorded; the pass condition of 3.5 at 32 MiB; the instruction gate (the clamp is inactive without a cap) | first: an embedder with a memory budget, or the maintainer | 6–8 days |
| Dynamic code | reclaiming dynamic units (3.3) | the three corpus programs; Code-Load's peak RSS within 2× QuickJS; the per-frame cost on the eight under the gate, or the boundary fallback recorded | with the heap cap, or when `eval`/`Function` churn is reported | 5–8 days |
| Code images | `encode_image`, `load_image`, `--compile`, `.oxc` programs; the fuzz-style flip test (4.1, 4.2, 4.4) | every suite program's image loads and prints the manifest's stdout; the validator rejects a flipped image at every offset without a panic; the image rows of the snapshot and image table | when a device port is scheduled | 6–8 days |
| Heap snapshots | writer, loader, quiescence, fingerprint, truncation test, `--snapshot`, `--load-snapshot` (4.2–4.4) | the realm round trip is byte-identical across two runs and loads to a runtime that passes `core/tests/corpus`; a snapshot after each of the three setup corpora runs its program to the manifest's stdout; the gate of 4.5 for the realm | when a port asks for a warm start | 8–12 days |
| 32-bit | the i686 gate and the audit (5.1, 5.2); the `suite-32bit` tier | section 5.1's acceptance; the audit in the pull request; the 32-bit table | with code images, or when a 32-bit port is scheduled | 3–5 days |
| `no_std` | `oxide-core` without `std` (5.4); the recipe in `core/README.md` (5.3) | the `thumbv7em-none-eabi` check in `fast`; `--no-default-features` tests; the instruction gate with the core's own table; receipts unchanged or re-frozen | when a port without `std` is scheduled | 8–12 days |
| Measurement | each item's tables on the reference machine; the fixed-work suite tables and the Test262 receipt replayed | section 9's exit conditions | per item, as each lands | 4–6 days |
| Generational collector | section 6, if not struck | the go/no-go of section 6 recorded either way | only on the maintainer's request | 10–15 days |

Total, if every item is taken: about 39–58 working days without the generational collector, 49–73 with it. The order is the priority: the heap cap and dynamic code are what any embedder with a memory budget asks for first and need no port to exist; code images are what a device port needs before anything else (compiling on the host); heap snapshots and `no_std` wait for a port that asks for a warm start or has no `std`; 32-bit goes with code images or with the first 32-bit port; measurement is done per item as items land; the generational collector is read against the finished memory, capped-run, snapshot and 32-bit tables, if it is taken at all. Each item is its own exit (section 9).

## 9. Measurement and exit

Every number is produced by [Measurement method](performance/measurement.md) on the reference machine: the frozen manifest, `fixed.py --repeat 5 --cpu 2 --linux-counters --timeout 900`, medians of 5, every sample's stdout checked, raw samples kept outside the repository; probe slopes from `docs/performance/probes/slopes/instruction_slopes.py`; instruction counts from `perf stat -e instructions:u`.

**The memory table: the 37 programs** at the default trigger: the `MemoryReport` at exit (reserved and live by arena, code store, value heap), peak RSS for QuickJS and `oxide-core`, and the reserved-to-live ratio before and after the heap cap item's shrinking.

**The capped-run table.** The 37 programs at one `run()`, the 20 probes at smoke size and the churn ring, under 32, 24 and 16 MiB (8 MiB for the ring), on x86_64 and i686: completion or `OutOfMemory`, collections, total and maximum pause, heap reserved, peak RSS, and time against the uncapped run.

**The snapshot and image table** as 4.5 specifies, with sizes.

**The 32-bit table.** The eight programs and the 20 probes on `i686-unknown-linux-musl` against x86_64: instructions and time; the `size_of` table of 5.2.

**Replayed with each item's final binary:** the fixed-work V8 v7, Octane and AWFY tables against QuickJS, the `call0` and `array_write` probe slopes, and the full Test262 receipt.

**Exit condition, per item.** Each item lands with its table in its pull request and with the replayed tables on its final binary; `gc-stress` green on the corpus, the probes and the 37 programs (x86_64, and i686 once the 32-bit item has landed). The conditions below belong to the items named; an item not taken leaves its condition open, not failed:

- **Memory.** The pass condition of 3.5 holds at 32 MiB on both targets; Code-Load's peak RSS is within 2× QuickJS's; no program's reserved-to-live ratio after a collection is above 3 except within one growth step of a collection.
- **Snapshots.** The realm snapshot is deterministic, width-independent and loads below half the cold initialization's instructions; every suite program's image loads below a tenth of its compilation's instructions and prints the manifest's stdout; the truncation and flip tests end in `Err` at every offset.
- **32-bit.** Section 5.1's acceptance holds in the nightly tier; the `no_std` check is in `fast`.
- **No regression.** No V8 v7 program retires more than 2% more instructions than the base branch's binary; the Test262 receipts replay (or are re-frozen with the reason recorded); `run_at` is under the loop-size gate.
- **The collector.** Section 6's go/no-go is recorded, or the section's striking is.

Failing a memory or snapshot gate is a finding that names the mechanism, recorded in the pull request; the item does not land until the gate holds or the maintainer accepts the exception in writing there.

## 10. Decisions

Made:

- the items are taken by need, in section 8's order;
- the trigger keeps a 4 MiB minimum and growth 2; under a cap it is clamped (3.1);
- the heap stays non-moving and gives back trailing free capacity (3.2);
- dynamic units are reclaimed at the collection, through per-slot cells, with the boundary fallback only on a measured gate failure (3.3);
- static units live as long as the runtime; a snapshot writes them whole (3.3, 4.2);
- two artifacts, images and snapshots, in one little-endian `u32`-indexed format with a build fingerprint; hash tables rebuilt at load; inline caches empty at load; loaders treat input as untrusted and the publication validator runs on every image (4.2);
- a snapshot requires quiescence and holds no host state (4.3);
- the 32-bit gate target is `i686-unknown-linux-musl`, with `wasm32` as the second 32-bit build; MIPS targets are left to the ports' nightly recipe (5.1, 5.3);
- `oxide-core` keeps zero dependencies beyond `oxide-regexp`: its own hash table, its own exact rounding functions, transcendental math through the `Host` in `no_std` builds (5.4);
- the generational option is sticky mark bits with a write barrier in the `Heap`'s write entry points, behind a feature until its go/no-go (6).

Open, with the item that decides each:

- whether reclamation at the collection or at the runtime boundary ships: dynamic code, by the instruction gate;
- the shrink thresholds (a quarter trailing, half free, `bytes > 2 × live`): heap cap, by the memory table's ratios and the churn ring's steady state;
- whether `oxide-regexp` needs a `std` feature: `no_std`;
- whether a non-gating weekly job runs the PSP recipe with a nightly toolchain: the maintainer, with the `no_std` item;
- whether a snapshot may be taken with live handles (writing them as a handle section) for embedders that hold values across the snapshot: heap snapshots, only if a consumer asks; the default is to require none;
- the young threshold and the stress intervals of the generational mode: the generational collector;
- the generational collector itself: its go/no-go, or struck by the maintainer before it starts.

## 11. Risks

- **A dangling unit.** Reclaiming code the loop may still be running is the one way to make the register VM read freed instructions. Mitigation: the per-slot borrow makes a running unit unfreeable by construction (the sweep cannot take the cell); the frame table is a root for units as it is for functions; the stress mode runs the dynamic corpus at interval 1.
- **Snapshot drift.** Every record field or store added later must be written and read, or snapshots silently lose state. Mitigation: the writer and reader are generated from one description per record (a `Serial` trait implemented once per type, with a round-trip test that compares the `MemoryReport` and the corpus output before and after), and the fingerprint rejects a stale file rather than loading it.
- **Determinism of a snapshot.** Hash-table iteration order could make two snapshots of one runtime differ. Mitigation: tables are rebuilt at load rather than written (4.2); everything written is in index order; the byte-identity test is in `fast`.
- **The barrier's cost.** A write barrier on the `SetField` hit path is paid by every program for Splay's benefit. Mitigation: the go/no-go's 2% gate on the eight, and removal rather than a feature if it fails.
- **`no_std` reach.** The split touches many files for a build this repository cannot run end to end. Mitigation: one implementation for both builds wherever possible (the table, the rounding functions), `std`-gated modules rather than `cfg` inside functions, and the `thumbv7em` check in `fast` so the build cannot rot.
- **Math parity.** Routing transcendental functions through the `Host` could change a result if a default differed from the standard library's call. Mitigation: the `std` default is the direct call, the receipts are re-frozen only with the reason recorded, and the Test262 full vector runs nightly.
- **Shrinking that thrashes.** Giving memory back and taking it again on every collection costs page faults. Mitigation: the `bytes > 2 × live` condition and the churn ring's steady-state measurement in the heap cap item.
- **The i686 tier's time.** The 37 programs under stress take 93 minutes on x86_64, and a 32-bit build is slower. Mitigation: the 32-bit item measures and sets the tier's stress intervals.
