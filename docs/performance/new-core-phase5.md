# New core, phase 5: memory and embedding targets

This page is the implementation plan for phase 5 of the new core tracked in issue #122 ("optional copying nursery; heap cap policy; position-independent heap snapshots and bytecode precompilation (indices need no pointer rewriting); 32-bit MIPS build", exit: "controlled workloads under PSP-class memory; snapshot load time"). Like the [phase 1](new-core-phase1.md) to [phase 4](new-core-phase4.md) plans it is written for whoever implements the phase: it fixes the decisions that are made, lists the ones that are deliberately open with the milestone that decides each, and gives every milestone an acceptance check that can be run without judgment calls. Day estimates order the work; they are not commitments.

**Order (the maintainer's decision of 2026-10-17): [phase 6](new-core-phase6.md) runs first.** Nothing in this phase has a consumer today, every item is cheaper once the old core is gone and a single engine is left in the tree, and the embedding API phase 6 fixes is what these items plug into. This phase is therefore not one block of work but a list of items taken by need, in the priority order of section 8; the page keeps the whole design so that each item can be picked up without re-planning. Two small items moved into phase 6 because the API needed them: `RuntimeOptions` (3.4) and `MemoryReport` (2). Where this page says `qjs`, it means the one binary phase 6 leaves (`oxide-run` is folded into it).

Read first: [#122](https://github.com/pocket-nexus/quickjs-oxide/issues/122) for the design, the [phase 6 plan](new-core-phase6.md) for the API, `RuntimeOptions` and `MemoryReport` this phase builds on, the [phase 4 results](new-core-phase4-results.md) for the exit tables this phase starts from (Table D in particular), [phase 1](new-core-phase1.md) §3–4 and the [phase 1 results](new-core-phase1-results.md) M5–M6 for the heap, the cap and the churn measurement, [`core/README.md`](../../core/README.md) for the rooting rule and every departure from the earlier plans, and [Measurement method](measurement.md).

## 0. Scope

**In scope.** The heap cap as a policy rather than a check (the trigger under a cap, growth that stops at the cap, giving memory back, the out-of-memory error); reclaiming the code of dead dynamically compiled functions; runtime options for small targets (heap, register file, native stack); code images (lowered units serialized without heap state, loaded into any runtime of the same build) and heap snapshots (the whole runtime at a quiescent point, loaded in place of initialization); 32-bit correctness and memory on a tier-2 32-bit target; `oxide-core` without the standard library, for the console ports; the measurement of section 9. The generational collector of section 6 is in scope as an optional, measurement-gated milestone.

**Out of scope.** The public embedding API, `qjs`, the adapters and the deletion of the old core (phase 6, done before this phase). A JIT. Linking for the PSP itself: its Rust target is tier 3 and needs a nightly toolchain with `-Zbuild-std`, which this repository does not use (section 5.3 says what this phase delivers instead and how the port consumes it). Any change to the per-program floor, the loop-size gate or the instruction gate, which this phase keeps as phase 4 left them.

**The maintainer's decisions of 2026-10-17**, recorded here because this phase measures against them:

- the 3.0 floor extends to Octane and AWFY as phase 4 §10 wrote it (V8 v7 RegExp, Octane RegExp and Code-Load excluded as shared-component programs); the phase 4 reads of 3.68 / 3.84 / 5.69 are the comparison base;
- `call0` stays at 1.43× QuickJS's slope. Phase 4 M1 found the remaining gap in the safe indexing the project keeps; no further call-path work is planned, and the call sequence is not touched by this phase except as the instruction gate requires;
- the collector's trigger stays at a 4 MiB minimum and growth 2. The generational collector of section 6 is an addition behind that trigger, not a change to it, and the maintainer may strike section 6 at phase start.

**What phase 4 learned that shapes this phase.** The exit tables put the new core at or below QuickJS's time on most programs; the memory side is not there yet:

| Program | Peak RSS QuickJS / current / new | Heap reserved | Live at exit | Collector share |
| --- | --- | ---: | ---: | ---: |
| Splay (both) | 144 / 312 / 265 MiB | 293.6 MiB | 75.0 MiB | 33–34% |
| Code-Load | 19 / 53 / 121 MiB | 5.9 MiB | 1.4 MiB | 0.7% |
| Mandreel | 57 / 103 / 100 MiB | 1.4 MiB | 1.0 MiB | 0.0% |
| PdfJS | 22 / 81 / 68 MiB | 43.2 MiB | 5.6 MiB | 6.8% |
| TypeScript | 98 / 237 / 90 MiB | 95.7 MiB | 20.3 MiB | 0.9% |
| Havlak | 33 / 106 / 41 MiB | 49.6 MiB | 8.3 MiB | 11.1% |
| zlib | 134 / 148 / 20 MiB | 1.4 MiB | 356 KiB | 0.0% |

Three mechanisms explain the table, and each is a section below:

- **Reserved memory never comes back.** Arenas and the value heap are `Vec`s that grow by doubling and never shrink; Splay reserves 294 MiB for 75 MiB live, Havlak 50 for 8. Under a cap this is the difference between fitting and `OutOfMemory` (section 3).
- **Code is never freed.** Code-Load's heap is 6 MiB and its process 121 MiB: the units `eval` and `Function` publish, their source text and their position tables live as long as the runtime. Mandreel's 100 MiB is the units of its multi-megabyte source plus the buffers the program keeps; its heap is 1.4 MiB (section 3.3).
- **Every collection marks the whole live set.** Splay spends a third of its time in nine collections of 86–115 ms each over a 75 MiB live set (section 6).

And one thing already true that this phase relies on: an empty script costs the new core **3.04 million** user-space instructions and 16 MiB of RSS on this tree's `oxide-run` (the old engine's `qjs`: 7.78 million, 16 MiB), of which the RSS is the process floor both share. The realm's initialization is what a heap snapshot replaces (section 4).

## 1. Location

| Module | Crate | Content |
| --- | --- | --- |
| `heap::gc` | `oxide-core` | the cap-aware trigger, growth that stops at the cap, shrinking after a collection (3.1, 3.2); the generational option (6) |
| `heap::report` | `oxide-core` | `MemoryReport` (built in phase 6; section 2 is its contract) |
| `bytecode` | `oxide-core` | reclaiming dynamic units (3.3); the code image's unit encoding (4.2) |
| `snapshot` (new) | `oxide-core` | the snapshot writer and loader over every arena and store (4.3) |
| `runtime` | `oxide-core` | the behavior behind `RuntimeOptions` (3.4); `Runtime::snapshot`, `Runtime::from_snapshot`, `Runtime::load_image` (4.4) |
| `host` | `oxide-core` | the `std` feature split: streams, clock and file access as `std`-only defaults; the math provider (5.4) |
| `heap::hash`, `shape`, `heap::atom`, … | `oxide-core` | the core's own hash table in place of `std::collections::HashMap` (5.4) |
| `api` | root crate | `--compile`, `--snapshot`, `--load-snapshot` for `qjs`; the image encoding of `CompiledUnits` |
| `apps/cli` (`qjs`) | CLI | the options above |
| `scripts/benchmark`, `probes/new-core` | tools | Table F–I runners (section 9) |
| `.github/workflows/ci.yml` | CI | the `i686` and `no_std` checks in `fast`; the `suite-32bit` nightly tier (section 7) |

Delivery: one pull request per milestone, stacked in milestone order, each with its acceptance check in the description, gated by `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `cargo test --locked --workspace --all-targets`, `cargo test -p oxide-core --features gc-stress`, `core/tests/corpus` under `gc-stress`, `check-new-core-stress.py`, the loop-size check (`vm::interp::run_at` under 16,384 bytes; it is at 16,021), and the per-pull-request instruction gate: `instructions:u` of the eight V8 v7 programs against phase 6's final `qjs`, a rise above 2% explained by a profile or fixed before merge. The Test262 focused vector replays in `fast` as today, and an item that changes an observable behavior (3.4's error, 5.4's math) re-freezes the full receipt.

## 2. Memory accounting

`Runtime::memory_report() -> MemoryReport` is built in phase 6 (its §2.4) as the replacement for the old `HeapCounts` and the profiling build. This section is its contract, kept here because it is the one source of every memory number this phase reports and the embedder reads. It costs a walk of the arena bitmaps and the free lists, never a collection:

| Field | Content |
| --- | --- |
| per arena (objects, shapes, strings, symbols, bigints, cells, functions, bytecode, internals) | live records, capacity, bytes reserved (`capacity × size_of`), bytes of out-of-line data the records own (string units, BigInt limbs, buffers, tables, suspended frames) |
| value heap | slots in use, free slots by size class, capacity, bytes reserved |
| ropes | `rope_bytes` |
| atoms | count, table capacity |
| code store | static and dynamic units, `Insn` bytes, handler and capture bytes, position-table bytes, source-text bytes |
| regular expressions | records, program bytes |
| register file | capacity, high-water mark |
| inline caches | entries |
| collector | `GcStats` (collections, total and maximum pause, freed records and bytes, live bytes after the last collection), the current threshold, the cap |

`Heap::bytes()` (what the cap compares against) is the sum of the reserved columns; the report states the identity so the two cannot drift. `qjs --memory-report` prints it at exit as `--gc-stats` prints the collector's, and `fixed.py` keeps reading peak RSS from the operating system as Table D did; the two are reported side by side, since the process floor (16 MiB on an empty script) and the allocator's own overhead are outside the heap's view.

**Acceptance (phase 6 M1, then this phase's M1).** Phase 6 lands the exact-count test and the `Heap::bytes()` identity under `gc-stress`; M1 of this phase records the report of each of the 37 programs at one `run()` as the first column of Table F before it changes anything.

## 3. The heap cap as a policy

### 3.1 The trigger under a cap

Today `safepoint` compares `allocated + nominal` against `threshold = max(min_heap, live × growth)` and, when a cap is set, additionally checks `bytes + growth_bytes(need) > max_bytes` and collects or fails. Growth 2 over a 75 MiB live set means the next collection comes at 150 MiB whatever the cap says, so a 128 MiB cap turns every Splay allocation past 128 MiB into a collection (the `over` check fires on each one once the heap sits at the cap). The rule becomes:

```text
threshold = clamp(live × growth, min_heap, cap − headroom)      when a cap is set
headroom  = the largest single growth step any arena or the value heap would take next
```

so that under a cap the collection happens before the heap reaches it, and the heap sits below the cap by one growth step rather than at it. `growth` stays 2 and `min_heap` 4 MiB (the maintainer's decision); only the clamp is new, and it is inactive without a cap, so Table A is unchanged by construction (the instruction gate confirms it).

### 3.2 Growth that stops at the cap, and shrinking

- **Growth.** An arena or value-heap growth step that would cross the cap grows to the cap instead of doubling (phase 1 M6's noted refinement). `Arena::growth_bytes` and `ValueHeap::growth_bytes` already compute the step; the step is clamped where it is taken.
- **Shrinking after a collection.** After a sweep, every arena whose trailing free run is more than a quarter of its capacity and whose free share is above half gives the trailing run back (`Vec::truncate` plus `shrink_to`; the free list drops the indices above the new length). The value heap does the same with free ranges that end at its high-water mark; a non-moving heap can give back only what is free at the top, and M1 records what that recovers on Splay, Havlak, PdfJS and TypeScript. Shrinking only runs when `bytes > 2 × live` after the collection, so the steady state of a program that fills and refills the same heap (the churn ring of phase 1 M6) pays nothing.
- **The register file** grows as it does today and is capped by `RuntimeOptions.stack_limit` (3.4); it shrinks back to its initial reservation at the runtime boundary when the frame table is empty and it is above four times its initial size.

### 3.3 Reclaiming dynamic units

A unit published at run time (`eval`, the `Function` constructors, dynamic `import()` of a new module) lives in `CodeStore.dynamic`, write-once chunks published through `&self` because the main loop holds the running unit's `&CodeUnit` across heap calls, and nothing frees it. A `FunctionBytecode` record (a heap record, traced) names its unit, so after marking the heap knows which dynamic units no live function and no running frame reaches.

Decision: **dynamic units are reclaimed at the collection that finds them dead, with one cell per unit.** `CodeStore.dynamic`'s slots become per-slot cells (`RefCell<Option<Box<CodeUnit>>>`) instead of the chunk's `OnceCell`: the loop borrows the running unit's slot for the frame's duration (one borrow-flag increment at frame entry and one decrement at return, on dynamic units only; static units keep today's plain `&CodeUnit`), and a dead unit's slot is by definition unborrowed, so the sweep frees it and returns its inline-cache range to the `IcStore` for reuse (the store gains a free list of ranges; today `ic_len` only grows). A freed slot is reused by the next dynamic publication. Static units (published before the program runs) are never reclaimed: they are the program, and section 4's snapshot serializes them as a whole.

If M2 measures the per-frame borrow on the eight above the 2% gate (Code-Load is the only suite program that runs dynamic units in its hot path; the eight run none, so the expected cost is the kind test at frame entry), the fallback is reclamation at the runtime boundary (`run_script`, `run_jobs`, the host's entry points), where `&mut CodeStore` is available with no loop running; M2 records which was built and Code-Load's peak RSS under each.

**Acceptance (M2).** A program that compiles 10⁵ functions through `new Function` and keeps none has a steady `MemoryReport` code-store size and a steady peak RSS under a 32 MiB cap; a program that keeps every tenth one keeps exactly those units; a function returned from `eval` and called after the collection that freed its siblings runs (`core/tests/corpus` gains both); Code-Load's peak RSS is within 2× QuickJS's (19 MiB) in Table F.

### 3.4 Runtime options and the out-of-memory error

```rust
pub struct RuntimeOptions {
    pub heap: HeapLimits,            // max_bytes; usize::MAX for no cap (today's)
    pub min_heap: usize,             // 4 MiB
    pub growth: f64,                 // 2.0
    pub stack_limit: u32,            // registers; 2^20 today (DEFAULT_STACK_LIMIT)
    pub native_stack_budget: usize,  // bytes of host stack re-entrant calls may use; 1 MiB today
    pub initial_registers: u32,      // the register file's first reservation
}
```

`RuntimeOptions` and the `qjs` flags `--heap-cap-mib`, `--min-heap-mib`, `--gc-growth` and `--stack-limit` exist from phase 6 (its §2.4), with today's behavior behind them; this item is the behavior: the clamp of 3.1, the growth and shrink rules of 3.2, the register file's limit and shrink, and `initial_registers` as a new field. A PSP-class profile is one `RuntimeOptions` value, documented in `core/README.md` (a 24 MiB cap, a 1 MiB minimum heap, a 2¹⁶-register file, a 256 KiB native budget), not a build configuration.

`HeapError::OutOfMemory` is already thrown as the realm's `%OutOfMemoryError%` intrinsic (`InternalError: out of memory`, QuickJS's). What changes: every allocating opcode and native must reach `vm::error::oom` rather than unwrap, the thrown value must not itself allocate (the intrinsic is preallocated, as today), and the error is catchable, after which the heap is usable again (the failed allocation was never made). A `try { … } catch` loop that fills the heap to the cap and recovers 100 times is the M1 test, under `gc-stress`.

### 3.5 PSP-class controlled workloads

The "controlled workloads under PSP-class memory" of #122's exit are fixed here: the 37 suite programs at one `run()` and the 20 fixed probes at smoke size, under caps of 32, 24 and 16 MiB, plus phase 1 M6's churn ring (`workloads churn`, a 12–16 MB live set) under an 8 MiB cap once the ring is sized to 4 MiB live, on x86_64 and on the 32-bit target of section 5, each recording completion or `OutOfMemory`, collections, pauses, heap reserved and peak RSS (Table G). The pass condition is objective: under a cap of C, every program whose live set at exit in phase 4's Table D is below C/2 completes with the manifest's stdout, and its peak RSS is at most C plus the process floor plus 10%. Splay (75 MiB live) is expected to fail every cap and is reported, not excused; QuickJS cannot run it in 32 MiB either.

## 4. Code images and heap snapshots

### 4.1 Two artifacts

| Artifact | Content | Loaded into | Replaces |
| --- | --- | --- | --- |
| **code image** (`.oxc`) | the lowered units of one script or module graph, with their constants by value (atoms by text, numbers, BigInt digits, regular expressions as source and flags), no heap state | any runtime of the same build, by interning the constants and publishing the units | parsing, compiling and lowering the source; the front end need not be on the device |
| **heap snapshot** (`.oxs`) | every arena, the value heap, the atom table, the shapes, the realm, the code store and the regular-expression store of a runtime at a quiescent point | a new runtime, in place of `Runtime::new` and whatever setup scripts ran before the snapshot | the realm's initialization (3.04 M instructions) and the program's own setup |

A device port compiles on the host (`qjs --compile`) and ships images; `eval`, the `Function` constructors and dynamic `import()` of source then throw `InternalError: no compiler` (the compile host is absent), which the deviation ledger records for such builds. A port that wants a warm start ships a snapshot of the realm plus its framework.

### 4.2 Position independence and the format

Every cross-record reference in the core is an index (`ObjectId`, `StringId`, `ShapeId`, …, a `Value` payload, a `CodeUnitId`, an `IcStore` site), never an address; a native function is the discriminant of the `Native` enum; a bytecode record names its unit by index. A snapshot therefore writes each arena at its current indices and nothing is relocated at load. The format:

- little-endian; every count and index is a `u32`; every `Value` is its `u64`; `f64` as its bits; no `usize` anywhere, so the bytes are identical from a 32-bit and a 64-bit build of the same source (the M5 test);
- a header with a magic, a format version (a constant bumped by hand) and a **build fingerprint**: a hash of the `Opcode` and `Native` name tables and of the record sizes, computed once at startup, so a file from another build is rejected with `SnapshotError::Fingerprint` before any of it is read;
- per arena: the live bitmap, then the live records in index order; the loader fills dead indices into the free list from the bitmap. Record fields that own Rust allocations (string units, BigInt limbs, buffers, the shape's transitions and lazy table, the collection tables) are written as their contents and rebuilt as allocations by the loader;
- hash tables are not written; they are rebuilt at load from what they index: the atom table from the records flagged `is_atom` (their stored `hash` is written, so rebuilding is insertion without rehashing), `shape_roots` from the shapes without a parent, each shape's `transitions` from its children's parent links, `prototypes` (cells and dependents) from the shapes' prototype fields and the cell arena, the regular-expression index from the records. A shape's lazy property table stays unbuilt until the first lookup that wants it;
- the inline caches are not written: the loader reserves each unit's `ic_count` entries empty, in publication order, so `ic_base` is unchanged;
- the code store writes static and dynamic units alike (a `Function`-made function live in the heap keeps its unit), each as its `Insn`s, handlers, captures, children, flags, counts, name atom, length, the position table and, unless the snapshot is stripped, the source text; `DebugInfo.filename` goes through one string table;
- ropes are flattened by the writer (a snapshot runs a full collection first, which the writer triggers; flattening is what the next operation on the rope would do anyway);
- a `SharedArrayBuffer`'s bytes are written as bytes (sharing is a property of a running process); detached buffers write their flag;
- the collector's state is not written beyond `live_bytes`; the loaded runtime starts with `allocated = 0` and today's threshold rule.

A code image is the code-store section of the format plus a constant table (the strings, numbers, BigInts and regular expressions the units name) and the tree `Runtime::instantiate` takes; loading publishes through `CodeStore::publish`, so the **publication validator runs on every image**: a malformed image fails at load with the validator's error, never at run. Template-site objects, which the lowering caches in the constant table once a tagged template is evaluated, are written as empty and rebuilt on first use.

### 4.3 Quiescence

`Runtime::snapshot` requires and checks: no frame on the register file, an empty job queue, an empty kept-objects list, no live handles, no agent threads, and no `Atomics.wait` waiter. Anything else is `SnapshotError::NotQuiescent(reason)`. Suspended generators and async activations are heap records and are written like any object; a pending promise is data; a `WeakRef` whose target died reads `undefined` after the load as it would after a collection. The host's state (`Host`, `Services`) is never part of a snapshot; the loader takes a fresh `Host`.

### 4.4 API and driver

```rust
impl Runtime {
    pub fn snapshot(&mut self, options: SnapshotOptions) -> Result<Vec<u8>, SnapshotError>;   // strip_source: bool
    pub fn from_snapshot(bytes: &[u8], host: Box<dyn Host>, options: RuntimeOptions) -> Result<Runtime, SnapshotError>;
    pub fn load_image(&mut self, bytes: &[u8]) -> Result<ObjectId, LoadError>;                  // the script or module function
}
```

The root crate's `api` gains `encode_image(CompiledUnits) -> Vec<u8>` beside its lowering entry, and `qjs` gains `--compile OUT file.js` (an image of one script; with `--module`, of a module graph resolved through the exact-module loader), `--snapshot OUT [setup.js …]` (a realm, or a realm after the setup scripts, with `--strip-source` honored), `--load-snapshot FILE` (start from it instead of `Runtime::new`), and runs a `.oxc` argument through `load_image`. Images and snapshots are untrusted input to their loaders: every length is bounds-checked against the buffer, every index against the arena it names, and the two fuzz-style tests of M3 and M4 (truncation at every byte offset of a realm snapshot, and a byte flip at every offset of a small image) must end in `Err`, never a panic.

### 4.5 What is measured

Table H: for the realm alone and for the realm plus each of three setup corpora (the AWFY bundle, Octane TypeScript, Octane Mandreel: the largest sources, whose lowering phase 4 timed), the snapshot's size, the instructions and time of `from_snapshot` against those of `Runtime::new` plus compiling, lowering and running the setup; for each of the 37 programs, the image's size and the instructions of `load_image` against the compile-and-lower path (Code-Load excluded, since its work is compiling at run time). The gate: the realm snapshot loads in fewer than half the instructions of the cold initialization it replaces (below 1.5 M), and every image loads in fewer than a tenth of the instructions its compilation took.

## 5. 32-bit targets and the standard library

### 5.1 The 32-bit gate target

The maintainer's statement of the ports (PSP, Vita, 3DS, ESP-IDF) means MIPS and ARM 32-bit targets without a hosted operating system. On the pinned stable toolchain `rustup` offers no MIPS target at all (they are tier 3), and `mipsel-sony-psp` needs a nightly compiler with `-Zbuild-std`. The 32-bit gate therefore runs on **`i686-unknown-linux-musl`**: tier 2, statically linked with the toolchain's own C runtime, built and executed on the x86_64 reference machine and on CI's runners without an emulator. What it proves is what the ports need from this repository: `Value` is 8 bytes and `Insn` 8 bytes there as here, indices are `u32`, no `usize` reaches a format or a limit by accident, and the whole engine (core and front end) runs the corpus, the probes, the 37 programs and the focused Test262 vector on a 32-bit word. `wasm32-unknown-unknown`, already a release target through `apps/web`, is the second 32-bit build the `fast` job keeps checking.

**Acceptance (M5).** `cargo test -p oxide-core --target i686-unknown-linux-musl` with and without `gc-stress`; `qjs` built for the target runs `core/tests/corpus` and the 20 probes at smoke size under `--gc-stress=1` and the 37 programs at one `run()` under the nightly tier's intervals; `run-test262` built for the target replays the frozen focused vector; the realm snapshot and the AWFY image written by the i686 binary and by the x86_64 binary are byte-identical, and each loads the other's; Table I records the eight programs' instructions and time on i686 against x86_64 (the floor is an x86_64 measurement and is not applied to i686).

### 5.2 The audit

M5 includes a one-time audit, recorded in the pull request: every `usize` in `oxide-core`'s public types and in anything written to a file is justified or changed to `u32`/`u64`; every `as usize` from a `u64` is on a value known to fit; `size_of` assertions for `Value`, `Insn`, `Frame`, `ObjectHeader`, `StringRecord` and `Shape` are tests that run on both widths and record both numbers in `core/README.md`; `AtomicU64` (used by `Atomics` on 64-bit element kinds) is available on i686 and on the ports' cores, and the agents' `Condvar` path is `std`-only (5.4).

### 5.3 What the ports get

This phase does not link for a console. It leaves `oxide-core` buildable without the standard library (5.4), verified on a tier-2 target that has no `std` at all (`thumbv7em-none-eabi`, core and alloc only, installed by `rustup` on the pinned stable toolchain), and documents in `core/README.md` the recipe a port follows: a nightly toolchain, `-Zbuild-std=core,alloc`, the target's JSON or tier-3 name, a `Host` implementation over the SDK (streams, clock, time zone, file access, the math provider), `RuntimeOptions` for the device, and code images compiled on the host. Whether this repository also runs that recipe in a non-gating weekly job with a nightly toolchain is the maintainer's call (section 10).

### 5.4 `oxide-core` without `std`

`oxide-core` gets a default feature `std`; without it the crate is `#![no_std]` with `alloc`. What `std` provides today and what replaces it:

| Use | Today | Without `std` |
| --- | --- | --- |
| `Vec`, `Box`, `String`, `Rc`, `VecDeque`, `BTreeMap` | `std` | `alloc`, the same types |
| `std::collections::HashMap` with the core's `FxHasher` (shapes, transitions, the atom table, `shape_roots`, `prototypes`, ropes, ephemerons, the regular-expression index, two in the VM) | `std` | the core's own open-addressing table, generalized from `heap::table`'s `OrderedTable` (the Map and Set storage), with the same `FxHasher`; one implementation for both builds, so the `std` build is measured by the instruction gate (none of these tables is on a cache hit path) |
| `Instant` for pause timing | `std` | not timed; `GcStats` keeps counts and bytes |
| `Host` defaults: process streams, `SystemTime`, `read_to_string` | `std` | no defaults: the port implements `write`, `now_ms`, `local_tz_offset_minutes`; `read_file` defaults to `None` |
| threads, `Mutex`, `Condvar`, `mpsc` (the Test262 agents, a blocking `Atomics.wait`) | `std` | the `agent` module is `std`-only; `Atomics.wait` behaves as with `CanBlockIsFalse` (`TypeError`); `Atomics.notify` returns 0; `SharedArrayBuffer` keeps `Arc<[AtomicU8]>` (both in `alloc` and `core`) |
| `std::error::Error` impls | `std` | `core::error::Error` |
| `f64` transcendental and rounding methods (`sin`, `exp`, `ln`, `pow`, `sqrt`, `floor`, `round`, …; `Math`, `Date`, radix formatting, `Number.prototype.toFixed` and friends) | `std`'s, which are the platform's libm | the exactly specified ones (`floor`, `ceil`, `trunc`, `round`, `fmod` via `%`, `sqrt` correctly rounded) are the core's own bit-exact implementations in both builds; the transcendental ones go through `Host::math(op, x, y)`, whose `std` default calls the standard library, so the `std` build's results are today's and a port's are its SDK libm's, which is what QuickJS on that platform computes too |

The zero-dependency rule of the core stands: no `hashbrown`, no `libm`. `oxide-regexp` gets the same `std` feature if it needs one (it is a table-driven matcher over `u16` slices; M6 checks).

**Acceptance (M6).** `cargo check -p oxide-core --no-default-features --target thumbv7em-none-eabi` in `fast`; `cargo test -p oxide-core --no-default-features` on x86_64 (the tests that need threads or timing are `std`-gated); the instruction gate on the eight with the own hash table; the full Test262 receipt re-frozen if any `Math` or `Date` result changed (none should: the `std` default is the same library call).

## 6. The generational collector (optional)

#122 lists "an optional copying nursery, decided on measurements". Table D has the measurement: Splay spends 33% of its time in nine full collections over a 75 MiB live set, Havlak 11%, every other program under 8%. The lever is worth 20–25% of Splay's time, and nothing else in the table.

**A copying nursery is not the mechanism for this heap.** Moving a young object means renumbering it, and its index is held in the register file, the root stack, the handles, every object slot and element, the `IcStore`'s entries, the `shape_roots` and `prototypes` keys and the frame table; rewriting them all is the relocation pass the index design exists to avoid, and section 4 relies on indices never changing. The mechanism that fits a non-moving arena heap is **generational marking with sticky mark bits** (the design of CRuby's RGenGC and of Lua 5.4's generational mode): after a full collection the mark bits of survivors stay set and mean *old*; a minor collection marks from the roots and from the remembered set, stops at every marked object, and sweeps only the records allocated since the last collection (an allocation log per arena). What it needs:

- a **write barrier** on every store of a heap value into an object's slot, element, cell or table: `if marked(object) && value.is_heap() && !marked(value) { remember(object) }`, which on the `SetField` hit path is two bit loads and two compares. The stores: the `SetField` and `SetElem` hit and slow paths, `StoreCell`/`StoreCaptured`, `Heap::set_slot`, dictionary and `OrderedTable` writes, element pushes, the natives' writes through `property::set` and `define_own`. Missing one is a dangling index the stress mode finds, so the barrier lives in the few `Heap` entry points that write a value into a record, not at call sites;
- the **remembered set** (a `Vec<u32>` per arena, with a per-record flag so an object is remembered once), the **allocation log**, a per-arena *young* count feeding a minor trigger (`young_bytes ≥ young_threshold`, 1 MiB to start), and the existing full trigger unchanged (growth 2 over `live`);
- the stress mode and the verifier extended: under `gc-stress` every N-th allocation runs a minor collection and every M-th a full one, and the verifier checks after a minor collection that no old object points to an unmarked young one without being remembered.

**Go/no-go, measured at M8 and recorded in the results page.** Built behind a Cargo feature so the default binary is unchanged until the decision. Adopted (the feature becomes the default and is removed as a feature) only if all three hold: Splay's collector share drops below 15% and its time by at least 15%; no V8 v7 program retires more than 2% more instructions than phase 4's binary (the barrier's cost); every other program's time is within the noise phase 4 measured (±3%). Otherwise the code is removed in the same milestone, not left behind a feature, and the results page records the numbers. The maintainer may strike this section at phase start; sections 2–5 and the exit do not depend on it.

## 7. The driver and CI

`qjs` grows `--compile`, `--snapshot`, `--load-snapshot`, honors `--strip-source` for both, and accepts `.oxc` files as programs (`--memory-report`, `--min-heap-mib`, `--gc-growth` and `--stack-limit` come with phase 6). `workloads` (the phase 1 Rust micro-workloads) gains the cap and the report.

CI:
- `fast`: `cargo check -p oxide-core --no-default-features --target thumbv7em-none-eabi` (M6); `cargo check --target i686-unknown-linux-musl -p quickjs-oxide-cli` (M5); the snapshot round-trip, determinism and truncation tests run inside `cargo test -p oxide-core` (M4).
- `suite-32bit` (nightly and dispatch, from M5): build the i686 `qjs` and `run-test262` in release, run the corpus and the probes under stress, the 37 programs at one `run()` under the x86_64 tier's intervals, and the frozen focused vector; compare the realm snapshot with the x86_64 build's byte for byte.
- `suite-stress` (phase 6's name for the nightly suite tier) additionally runs the 37 programs at one `run()` under a 32 MiB cap (M1) and the churn ring under 8 MiB, checking the pass condition of 3.5.
- Reference-machine measurements (the instruction gate, Tables F–I) are recorded in pull requests, not run in CI, as in every phase.

## 8. Milestones

| # | Content | Acceptance | Taken when | Estimate |
| --- | --- | --- | --- | --- |
| M1 | Table F's first column; the cap-aware trigger, growth to the cap, shrinking, the register file's limit and shrink, the catchable `OutOfMemory` (3.1, 3.2, 3.4); the suite under a 32 MiB cap in `suite-stress` | the 100-recoveries test under stress; the shrink numbers for Splay, Havlak, PdfJS, TypeScript recorded; the pass condition of 3.5 at 32 MiB; the instruction gate (the clamp is inactive without a cap) | first: an embedder with a memory budget, or the maintainer | 6–8 days |
| M2 | reclaiming dynamic units (3.3) | the three corpus programs; Code-Load's peak RSS within 2× QuickJS; the per-frame cost on the eight under the gate, or the boundary fallback recorded | with M1, or when `eval`/`Function` churn is reported | 5–8 days |
| M3 | the code image: `encode_image`, `load_image`, `--compile`, `.oxc` programs; the fuzz-style flip test (4.1, 4.2, 4.4) | every suite program's image loads and prints the manifest's stdout; the validator rejects a flipped image at every offset without a panic; Table H's image rows | when a device port is scheduled | 6–8 days |
| M4 | the heap snapshot: writer, loader, quiescence, fingerprint, truncation test, `--snapshot`, `--load-snapshot` (4.2–4.4) | the realm round trip is byte-identical across two runs and loads to a runtime that passes `core/tests/corpus`; a snapshot after each of the three setup corpora runs its program to the manifest's stdout; the gate of 4.5 for the realm | when a port asks for a warm start | 8–12 days |
| M5 | the i686 gate and the audit (5.1, 5.2); the `suite-32bit` tier | section 5.1's acceptance; the audit in the pull request; Table I | with M3, or when a 32-bit port is scheduled | 3–5 days |
| M6 | `oxide-core` without `std` (5.4); the recipe in `core/README.md` (5.3) | the `thumbv7em-none-eabi` check in `fast`; `--no-default-features` tests; the instruction gate with the own table; receipts unchanged or re-frozen | when a port without `std` is scheduled | 8–12 days |
| M7 | each item's tables on the reference machine; Tables A, B and E replayed; the results page | section 9's exit condition | per item, as each lands | 4–6 days |
| M8 | the generational collector (section 6), if not struck | the go/no-go of section 6 recorded either way | only on the maintainer's request | 10–15 days |

Total, if every item is taken: about 39–58 working days without M8, 49–73 with it. The order is the priority: M1 and M2 are what any embedder with a memory budget asks for first and need no port to exist; M3 is what a device port needs before anything else (compiling on the host); M4 and M6 wait for a port that asks for a warm start or has no `std`; M5 goes with M3 or with the first 32-bit port; M7 is written per item as items land, not at the end; M8 is read against finished Tables F–I, if it is taken at all. Each item is its own exit (section 9): the phase has no single exit date.

## 9. Measurement and exit

Every number is produced as in phases 1–4: the [baseline](baseline.md)'s machine, the frozen manifest, `fixed.py --repeat 5 --cpu 2 --linux-counters --timeout 900`, medians of 5, every sample's stdout checked, raw samples kept outside the repository; probe slopes from `probes/new-core/phase2.py`; instruction counts from `perf stat -e instructions:u`.

**Table F: memory on the 37** at the default trigger: the `MemoryReport` at exit (reserved and live by arena, code store, value heap), peak RSS for QuickJS, the baseline's old-core `qjs` and the new core, and the new-core reserved-to-live ratio before and after M1's shrinking.

**Table G: under caps.** The 37 programs at one `run()`, the 20 probes at smoke size and the churn ring, under 32, 24 and 16 MiB (8 MiB for the ring), on x86_64 and i686: completion or `OutOfMemory`, collections, total and maximum pause, heap reserved, peak RSS, and time against the uncapped run.

**Table H: snapshots and images** as 4.5 specifies, with sizes.

**Table I: 32-bit.** The eight programs and the 20 probes on `i686-unknown-linux-musl` against x86_64: instructions and time; the `size_of` table of 5.2.

**Tables A, B and E replayed** with the final binary: the floor per program and per suite, `call0` and `array_write` beside their phase 4 values, the full Test262 receipt.

**Exit condition, per item.** Each item that is taken lands with its table in `new-core-phase5-results.md` and with Tables A, B and E replayed on the final binary of that item; `gc-stress` green on the corpus, the probes and the 37 (x86_64, and i686 once M5 has landed). The conditions below belong to the items named; an item not taken leaves its condition unwritten, not failed:

- **Memory.** The pass condition of 3.5 holds at 32 MiB on both targets; Code-Load's peak RSS is within 2× QuickJS's; no program's reserved-to-live ratio after a collection is above 3 except within one growth step of a collection.
- **Snapshots.** The realm snapshot is deterministic, width-independent and loads below half the cold initialization's instructions; every suite program's image loads below a tenth of its compilation's instructions and prints the manifest's stdout; the truncation and flip tests end in `Err` at every offset.
- **32-bit.** Section 5.1's acceptance holds in the nightly tier; the `no_std` check is in `fast`.
- **No regression.** The floor of the maintainer's decision holds per program and per suite; no V8 v7 program retires more than 2% more instructions than phase 4's binary; the new core's Test262 receipts replay (or are re-frozen with the reason recorded); `run_at` is under the gate.
- **The collector.** Section 6's go/no-go is recorded, or the section's striking is.

Failing a memory or snapshot gate is a finding that names the mechanism, recorded in the results page; M7 does not end until the gate holds or the maintainer accepts the exception in writing on that page.

## 10. Decisions

Made, and not reopened inside phase 5:

- phase 6 runs first; this phase is a list of items taken by need in section 8's order (the maintainer, 2026-10-17);
- `RuntimeOptions` and `MemoryReport` are phase 6's; this phase adds behavior behind them;

- the trigger stays at a 4 MiB minimum and growth 2; under a cap it is clamped, never changed (3.1);
- a non-moving heap gives back trailing free capacity only; no compaction, no relocation (3.2);
- dynamic units are reclaimed at the collection, per-slot cells, with the boundary fallback only on a measured gate failure (3.3);
- static units are never reclaimed; a snapshot writes them whole (3.3, 4.2);
- two artifacts, images and snapshots, in one little-endian `u32`-indexed format with a build fingerprint; hash tables rebuilt at load; inline caches empty at load; loaders treat input as untrusted and the publication validator runs on every image (4.2);
- a snapshot requires quiescence and never contains host state (4.3);
- the 32-bit gate target is `i686-unknown-linux-musl`; `wasm32` stays the second 32-bit build; no MIPS target in this repository's CI on the stable toolchain (5.1, 5.3);
- `oxide-core` keeps zero dependencies beyond `oxide-regexp`: its own hash table, its own exact rounding functions, transcendental math through the `Host` in `no_std` builds (5.4);
- a copying nursery is rejected for this heap; the generational option is sticky mark bits with a write barrier in the `Heap`'s write entry points, behind a feature until its go/no-go (6);
- `call0` is not worked on; the floor and the gates are phase 4's.

Open, with the milestone that decides them:

- whether reclamation at the collection or at the runtime boundary ships: M2, by the instruction gate;
- the shrink thresholds (a quarter trailing, half free, `bytes > 2 × live`): M1, by Table F's ratios and the churn ring's steady state;
- whether `oxide-regexp` needs a `std` feature: M6;
- whether a non-gating weekly job runs the PSP recipe with a nightly toolchain: the maintainer, at M6;
- whether a snapshot may be taken with live handles (writing them as a handle section) for embedders that hold values across the snapshot: M4, only if a consumer asks; the default is to require none;
- the young threshold and the stress intervals of the generational mode: M8;
- the generational collector itself: M8's go/no-go, or struck by the maintainer at phase start.

## 11. Risks

- **A dangling unit.** Reclaiming code the loop may still be running is the one way to make the register VM read freed instructions. Mitigation: the per-slot borrow makes a running unit unfreeable by construction (the sweep cannot take the cell); the frame table is a root for units as it is for functions; the stress mode runs the dynamic corpus at interval 1.
- **Snapshot drift.** Every new record field or store added later must be written and read, or snapshots silently lose state. Mitigation: the writer and reader are generated from one description per record (a `Serial` trait implemented once per type, with a round-trip test that compares the `MemoryReport` and the corpus output before and after), and the fingerprint rejects a stale file rather than loading it.
- **Determinism of a snapshot.** Hash-table iteration order could make two snapshots of one runtime differ. Mitigation: tables are not written at all (4.2); everything written is in index order; the byte-identity test is in `fast`.
- **The barrier's cost.** A write barrier on the `SetField` hit path is paid by every program for Splay's benefit. Mitigation: the go/no-go's 2% gate on the eight, and removal rather than a feature if it fails.
- **`no_std` reach.** The split touches many files for a build this repository cannot run end to end. Mitigation: one implementation for both builds wherever possible (the table, the rounding functions), `std`-gated modules rather than `cfg` inside functions, and the `thumbv7em` check in `fast` so the build cannot rot.
- **Math parity.** Routing transcendental functions through the `Host` could change a result if a default were ever written differently from the standard library's call. Mitigation: the `std` default is the direct call, the receipts are re-frozen only with the reason recorded, and the Test262 full vector runs nightly.
- **Shrinking that thrashes.** Giving memory back and taking it again on every collection costs page faults. Mitigation: the `bytes > 2 × live` condition and the churn ring's steady-state measurement in M1.
- **The i686 tier's time.** The 37 programs under stress took 93 minutes on x86_64; a 32-bit build is slower. Mitigation: M5 measures and sets the tier's intervals as M8 of phase 4 did.

## 12. What phase 6 leaves for this phase

Phase 6 precedes this phase and leaves: one engine in the tree and one binary, `qjs`, with `oxide-run`'s flags; the embedding API over `oxide_core::Runtime` with `RuntimeOptions`, `MemoryReport`, the unified `Host` and the module loader's `Scope`, which the snapshot and image entry points of 4.4 join as `Runtime` methods re-exported by the API; `oxide-core` as a separate crate, which the `no_std` build of 5.4 relies on; the Test262 receipts under `current.conf`, which every item here replays; the five deviation-ledger entries, which the sixth of 4.1 (a build without a compile host) would join. Nothing in this phase changes the API's shape beyond adding the entry points named in 4.4 and the `Host::math` method of 5.4.
