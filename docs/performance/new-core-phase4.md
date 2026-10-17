# New core, phase 4: builtins, strings and the full language surface

This page is the implementation plan for phase 4 of the new core tracked in issue #122 ("port builtins as Step/Resume state machines onto the new value type, ordered by Test262 and benchmark coverage; ropes and one-byte strings; fix iterator-result allocation in `Array.from`-style loops; Octane and AWFY running", exit: "Test262 back to v0.2.1 level minus dropped features; all three suites reporting"). Like the [phase 1](new-core-phase1.md), [phase 2](new-core-phase2.md) and [phase 3](new-core-phase3.md) plans it is written for whoever implements the phase: it fixes the decisions that are made, lists the ones that are deliberately open with the milestone that decides each, and gives every milestone an acceptance check that can be run without judgment calls. Day estimates order the work; they are not commitments.

Read first: [#122](https://github.com/pocket-nexus/quickjs-oxide/issues/122) for the design and the phase order, [phase 0](new-core-phase0.md) for the comparison base and the dropped language features, the [baseline](baseline.md) for the 37 suite programs the exit is measured against, the [phase 3 results](new-core-phase3-results.md) for the go/no-go and the notes it left for this phase, [`core/README.md`](../../core/README.md) for the rooting rule and the departures from the earlier plans, [`docs/test262.md`](../test262.md) for the Test262 metrics, and [Measurement method](measurement.md).

## 0. Scope

**In scope.** Everything that separates the new core from a complete engine within the surface phase 0 kept: the language constructs the lowering still declines (classes, generators, async functions, destructuring, non-simple parameters, iterators and `for-of`, template literals, object spread and literal methods, `super`, private names, `eval` and `new Function` with phase 0's indirect semantics, modules, the global lexical environment); the builtins the current engine implements, ported onto the 8-byte `Value` and the new heap; the object kinds they need (wrapper objects, typed arrays and buffers, collections, promises, proxies, dates, module namespaces); the job queue and the host boundary (clock, time zone, module loading, dynamic compilation); ropes; the Test262 runner on the new core; Octane and AWFY running on `oxide-run`; the performance work phase 3 left (`call0`, `array_write`); the measurement of section 10.

**Out of scope.** The five features phase 0 dropped (`with`, local `eval`, Realms, `Symbol.species`, unscopables): they are deviations, not phase 4 work, and section 7.2 fixes the Test262 variants they cost. The copying nursery, the heap cap policy, snapshots and the 32-bit build (phase 5). The public embedding API (`engine::api`) re-targeted at the new core, and the deletion of the old core (phase 6, section 13). A JIT. Any change to the current engine beyond the Test262 runner's second backend (section 7.1) and the hidden probe feature of section 7.2.

**What phase 3 learned that shapes this phase.** The [exit tables](new-core-phase3-results.md#m6-exit-tables) put the new core at 0.93× QuickJS's time over the eight V8 v7 programs and 3.63× the current core, with the lowest program (NavierStokes) at 3.05×. Three things follow:

- **The floor is now 3.0.** The maintainer raised the per-program floor from 2.5 to 3.0 on 2026-10-17 (section 10). NavierStokes has 1.6% of margin, and phase 3 M3 measured a 4–5% loss on every probe from the growth of `vm::run` alone. Phase 4 adds more opcodes than phases 2 and 3 together, so the loop-size discipline of section 4.1 is a gate on every pull request, not an M-last concern, and the margin work of M1 lands first.
- **The remaining gaps are in the call sequence and element writes** (`call0` 1.60×, `array_write` 1.49×), not in the caches, and Splay's collector is a third of its time. The first two are M1; the collector's lever is phase 5's nursery, and the trigger stays at growth 2 (section 11).
- **Natives already call back into JavaScript.** Accessors, `valueOf`, `toString` and the error paths run through the re-entrant `vm::call` with the 1 MiB native-stack budget; `apply` and `call` are rewritten in the loop. Section 2 decides the native ABI on that basis.

**The census.** What the 29 programs beyond the V8 v7 eight need, from the built programs of the baseline (`suite_matrix.py build` at the pinned sources, the bundles of the 2026-10-15 engine comparison), and what Test262 tests by area:

| Programs | Hot or required beyond phase 3 | Setup-only or never taken |
| --- | --- | --- |
| octane-richards … octane-navier-stokes (8) | the same code as the V8 v7 programs; Octane's RegExp is a revised program over the same engine | |
| octane-pdfjs | typed arrays (114 sites), `Object.defineProperty` getters, `bind`, `String.fromCharCode.apply`, `sort` with a comparator, `replace` with a function, `arguments`, `switch` | `Date`, `eval(next)` in the harness's timeout flush |
| octane-mandreel | typed arrays, 985 labeled loops, `arguments` (179), `switch`, `String.fromCharCode` | `eval(next)` in the timeout flush |
| octane-gameboy | typed arrays, `switch`, `in` | `new Function` probe |
| octane-code-load | indirect `eval` (`indirectEval = eval`) and `new Function` on large sources: its time is the shared front end | |
| octane-box2d | `apply`/`call` (217), `arguments` (189), accessors (18), `instanceof`, `parseInt` | |
| octane-zlib | typed arrays, `Math.imul`, `delete` (134), `eval.call(…)` (indirect), `arguments` | a direct `eval("_" + a)` inside `try` |
| octane-typescript | `arguments` (208), `bind`, `sort` with a comparator, `try/finally`, labeled loops, `replace` with a function, regular expressions, string building | `with` only inside string literals |
| awfy (14 programs, one bundle) | classes (79, with `static` methods and `super`), arrow functions, `let`/`const`, template literals, destructuring, `for-of` over arrays (Bounce), `new Set` (Havlak), `sort` with a comparator, `Error` subclasses | |

No suite program uses generators, async functions, Proxy, `Symbol` as a value, Map, WeakMap, Promise, modules or `with`. Every direct `eval` in the suites is either never taken or runs global code, where phase 0's indirect semantics give the same result.

Test262, files per area in the pinned suite (`test/`, `_FIXTURE` files excluded; a file runs as one or two variants), restricted to what the current engine admits:

| Area | Files | Area | Files |
| --- | ---: | --- | ---: |
| `language/statements/class` + `expressions/class` | 8,426 | `built-ins/Promise` | 652 |
| `built-ins/Object` | 3,411 | `built-ins/Date` | 594 |
| `built-ins/Array` | 3,081 | `language/module-code` + `import` | 715 |
| `built-ins/TypedArray` + `TypedArrayConstructors` + `Uint8Array` | 2,242 | `built-ins/DataView` | 561 |
| `built-ins/RegExp` | 1,879 | `built-ins/Iterator` | 510 |
| `language/statements/for-await-of` + `for-of` | 1,985 | `built-ins/Function` | 509 |
| `language/expressions/object` | 1,170 | `built-ins/Set` + `Map` + weak collections | 813 |
| `built-ins/String` | 1,223 | `built-ins/Atomics` + `SharedArrayBuffer` | 486 |
| `language/expressions/dynamic-import` | 941 | `built-ins/Proxy` + `Reflect` | 464 |
| generators and async (`expressions` + `statements`) | 1,573 | `annexB` | 1,086 |

`Temporal`, `intl402`, explicit resource management and the other `skipped-feature` areas are outside QuickJS 2026-06-04's scope and stay skipped. The port order of section 6 follows this table and the suite census together: the object model and classes first (they block the most tests), then iteration and suspension (they block the harness's async helpers and every collection builtin), then the builtins by test count, with typed arrays placed where Octane needs them.

## 1. Location

Everything new lives in `oxide-core` and the lowering, with two exceptions: the Test262 runner gains a second backend (section 7.1) and the current engine gains a hidden probe feature used once (section 7.2).

| Module | Crate | Content |
| --- | --- | --- |
| `vm::native` | `oxide-core` | the native ABI of section 2: `CallSite`, the rooting helpers, the re-entrant `call` |
| `vm::property` | `oxide-core` | the internal methods dispatched by object kind (section 3.1): ordinary, array, arguments, string wrapper, typed array, proxy, module namespace |
| `vm::iter` | `oxide-core` | the iterator opcodes' slow paths and the pristine-array fast path (section 4.2) |
| `vm::suspend` | `oxide-core` | generator and async frames: copy-out at `Yield`/`Await`, copy-in at resume (section 4.3) |
| `vm::jobs` | `oxide-core` | the job queue, promise reactions, the host's drain entry (section 5.4) |
| `heap::string` | `oxide-core` | the rope variant and flattening (section 3.3) |
| `heap::object` | `oxide-core` | the new kinds and their payload arenas (section 3.2) |
| `heap::gc` | `oxide-core` | ephemeron marking for weak collections, kept objects, finalization jobs (section 3.4) |
| `builtins` | `oxide-core` | one directory per builtin object, ported from `src/engine/builtins` (section 6) |
| `host` (new) | `oxide-core` | the `Host` trait: output, clock and time zone, module loading, dynamic compilation, `$262` hooks (section 6.3) |
| `modules` (new) | `oxide-core` | module records, linking, evaluation, namespaces, dynamic import (section 5.6) |
| `compiler::register` | root crate | the lowering of every construct in section 5 |
| `api::new_core` | root crate | the compile host (the front end plus the lowering behind the `Host` trait), the Test262 host, the CLI surface |
| `conformance/test262` | runner | `--engine new-core` (section 7.1) |

Gates that change: `scripts/checks/check-source-layout.py` does not check `core/`, so new directories under `core/src` need no admission; `test262-host-metadata.py` keeps `{cli, oracle, profiling, oxide_run}` as the CLI's integration targets and admits the runner's new backend as a feature of the existing `run-test262` target; `dev-support/test262/current.conf` is untouched by anything in this phase except the runner change of section 7.1, which is a pull request gated by the current engine's focused and full runs like phase 3's crate move was. Candidate A of the host-boundary check still holds: `oxide-core` depends on `oxide-regexp` and nothing else; the front end reaches it only through the `Host` trait object the root crate installs (section 6.3).

Delivery: one pull request per milestone, stacked in milestone order, each with its acceptance check in the description, gated by `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `cargo test -p oxide-core --features gc-stress`, the two-core corpus under `new-core-gc-stress`, `check-new-core-stress.py`, the loop-size check of section 4.1, the instruction gate of section 10 on the eight V8 v7 programs, and, for the two pull requests that touch code the current engine runs (sections 7.1 and 7.2), the Test262 focused and full runs of the current engine. From M9 the focused vector on the new core joins the `fast` job (section 8).

## 2. Natives that run JavaScript

#122 says "port builtins as Step/Resume state machines". The current engine needs them because a native cannot hold a `RefCell` borrow across a call, so every native that calls user code is a state machine the VM driver resumes with an owned reply; `continuation.rs`, `iterator/step.rs` and the `*Step`/`*Resume` pairs are a large share of the 86,769 non-test lines in `src/engine/builtins`. The new core has neither the borrow nor the driver: a native receives `&mut Cx`, roots what it holds on the root stack, and calls `vm::call`, which runs a nested loop on the frames above its own. Phase 3 built accessors, conversions, `instanceof` and the error paths on exactly that, and the stress mode checks the rooting.

**Decision: natives are straight-line Rust over the re-entrant `call`; the Step/Resume form is not ported.** Each native keeps the specification's step order and effect order (the old implementation is the reference for both, and its tests move with it), but its phases are sequential code, with every value that lives across a call or an allocation rooted. This is QuickJS's own model (`JS_Call` recursion on the C stack with a stack check), so observable behavior under deep recursion through natives matches the comparison engine: the 1 MiB native-stack budget throws `InternalError: stack overflow` as the register-file limit does. Consequences:

- The ABI is phase 3's `CallSite` plus three helpers: `root(v) -> Rooted` (a root-stack index that reads the current value), `call(f, this, args)` and `construct(f, args, new_target)`; the root stack is cut back after every native as today.
- Generators and async functions are not natives and never were: they suspend inside the loop (section 4.3). `Promise` reactions, `Array.from`, `forEach` and `sort` are ordinary natives.
- Phase 0's "`Trace` of pending native state machines" as a root set is superseded: the root stack is the root set, and `gc-stress` already walks it.
- A native must not cache an arena borrow across a call; this is the rule the stress mode enforces, and the port of each builtin runs under it from its first commit.

The alternative, the state machines with a resume table in the loop, was rejected for three reasons: it adds a second calling convention next to the one phase 3 already uses for accessors; its benefit (no native-stack growth) buys nothing against QuickJS, which recurses; and its port cost is the largest single item of the phase. The one place where a flat loop matters, iteration without allocation, is handled in the loop by section 4.2 instead.

## 3. The heap

### 3.1 Internal methods by kind

`property::get`, `set`, `has`, `delete`, `define_own`, `get_own`, `own_keys`, `get_prototype`, `set_prototype`, `prevent_extensions` and `is_extensible` become the single entry points of the object model, dispatching on `ObjectKind` before the ordinary algorithm. The inline hit paths are unaffected: a cache entry is filled only for an `Ordinary`, `Array` or `Function` receiver (and the string-receiver form), so a `Proxy`, `TypedArray` or `Arguments` receiver always takes the slow path, which dispatches. Kinds with exotic methods: `Array` (`length`), `Arguments` (the mapped `[[DefineOwnProperty]]` phase 3 deferred), `StringWrapper` (index and `length`), `TypedArray` (integer-indexed: canonical numeric keys never reach the shape), `Proxy` (every method through the handler, with the invariant checks), `ModuleNamespace` (own keys from the export list, writes rejected). `Reflect` is the natives over these entry points and nothing else.

Property enumeration order (`OwnPropertyKeys`) is one function: index keys ascending from the element range and from index-keyed named properties, then string keys in shape order, then symbols in shape order. `for-in`'s snapshot and `Object.keys`, `JSON.stringify`, `Object.assign` and the spread copy all read it.

### 3.2 Object kinds and payloads

`ObjectKind` grows from phase 3's nine to the kinds the builtins need. Payloads follow phase 1's rule: a kind with fixed-size per-object data beyond the header gets a side arena indexed by `payload`; a kind whose data is a few values stores them in raw slots (slots the shape does not describe, as `ForInIterator` does); a kind with variable-size non-value data gets a byte arena.

| Kind | Payload | Notes |
| --- | --- | --- |
| `StringWrapper`, `NumberWrapper`, `BooleanWrapper`, `SymbolWrapper`, `BigIntWrapper` | raw slot 0: the primitive | `new String("x")` and `Object("x")`; string indices are exotic (3.1) |
| `Date` | raw slot 0: the time value as a double | no arena |
| `Error` | the phase 3 flag stays; `stack` is an own data property built at construction from the frame table and `DebugInfo.lines` | QuickJS's `at f (file:line)` lines |
| `BoundFunction` | raw slots: target, bound `this`, then the bound arguments in the element range | the `Call` arm dispatches it like a native: push the bound arguments, call the target |
| `ArrayBuffer`, `SharedArrayBuffer` | `buffers: Arena<ByteBuffer>`; a shared buffer's bytes are an `Arc<[AtomicU8]>` as in the current engine, so `Atomics` and agents stay safe Rust | detach is a flag on the record; resizable buffers keep `max_byte_length` |
| `TypedArray` | `views: Arena<ViewRecord { buffer, byte_offset, length, kind, length_tracking }>` | the twelve element kinds the current engine has, `Float16Array` included; the `GetElem`/`SetElem` fast path is section 4.4 |
| `DataView` | the same record | |
| `Map`, `Set` | `tables: Arena<OrderedTable>`: an insertion-ordered open-addressing table over value-heap ranges, tombstones for deletion during iteration, compaction when tombstones exceed half | iterators hold the table and a cursor in raw slots and survive compaction through an epoch |
| `WeakMap`, `WeakSet` | the same table with ephemeron marking (3.4) | |
| `WeakRef`, `FinalizationRegistry` | raw slots; the registry's cells in a value-heap range | the kept-objects list and cleanup jobs are 3.4 |
| `Proxy` | raw slots: target, handler (`Value::NULL` once revoked) | callable and constructible flags copied from the target |
| `Promise` | `promises: Arena<PromiseRecord { state, reactions: Range }>`; the result in raw slot 0 | `is_handled` for the rejection tracker |
| `Generator`, `AsyncGenerator`, `AsyncFunction` | `frames: Arena<SuspendedFrame>` (4.3) | the async generator's request queue in the element range |
| `ArrayIterator`, `StringIterator`, `MapIterator`, `SetIterator`, `RegExpStringIterator`, `IteratorHelper`, `IteratorWrap`, `AsyncFromSyncIterator` | raw slots: the source and the cursor or state | `ArrayIterator` is the kind the fast path of 4.2 avoids allocating |
| `ModuleNamespace` | `modules: Arena<ModuleRecord>` (5.6) | |
| `GlobalLexical` | a second dictionary object like the global object, with its own generation counter | script-level `let`, `const` and `class` with a TDZ (5.7) |

Function objects get `length` and `name` as own data properties from a per-kind root shape laid out once (`length`, `name`, and `prototype` for constructors), filled at `MakeClosure` from the unit's metadata; `MakeClosure` is below 1% of every phase 3 profile, and the shape is shared, so the cost is two slot writes. The `FunctionRecord` gains `home_object: u32` for methods that use `super` and `fields: u32` for class constructors with instance fields (section 5.1).

### 3.3 Strings: ropes

`StringRepr` gains `Rope { left: StringId, right: StringId, len: u32, one_byte: bool }`. `ops::concat` returns a rope when the result is at least 64 code units and neither side is empty; shorter results are copied as today, which keeps Splay's two-piece keys flat (phase 3 M5 measured copying at 2% there, and a rope would add a record per key). A rope flattens in place on the first operation that needs its units (index, compare, hash, atom interning, regular-expression input, output): an explicit stack walks the tree, so a left-deep rope from a `+=` loop of depth n flattens in O(n) without recursion; after flattening the record is `OneByte` or `TwoByte` and its children are unreachable through it. The collector marks rope children; `length` and `one_byte` are read without flattening, so `s.length` and the one-byte concat path stay O(1). Atoms are never ropes. The trigger phase 3 §11 set stays as the measurement: TypeScript and PdfJS are the programs that build strings, and M5 records their instruction share in string work before and after.

### 3.4 The collector

- **Ephemerons.** A `WeakMap` entry's value is marked only once its key is marked. The mark phase keeps a list of weak tables reached; after the worklist drains it rescans them, marking values of marked keys, until no table changes; the sweep clears entries whose key is unmarked. `WeakSet` and `WeakRef` targets are weak the same way; a `WeakRef` whose target died reads `undefined`.
- **Kept objects.** `WeakRef.prototype.deref` and `new WeakRef` add the target to `Heap.kept` (a value-heap range rooted like the handles), cleared by the host at the end of the synchronous execution (`Runtime::clear_kept_objects`, called by the job drain). This is the deviation ledger's ECMASCRIPT-WEAKREF-LIFETIME-001 behavior, which the new core keeps.
- **Finalization.** After the sweep, every `FinalizationRegistry` cell whose target died queues a cleanup job; jobs never run inside a collection.
- **Suspended frames** are heap records (4.3) and are traced like scope objects.
- The trigger is unchanged (4 MiB minimum, growth 2); Table D measures the 37 programs.

## 4. The loop

### 4.1 Size discipline

`vm::run` is 14,600 bytes against the 16 KB gate. Phase 4 adds the opcodes of section 5. The rule, enforced in every pull request by the `nm --size-sort` check phase 3 used: a new opcode enters `vm::run` inline only when a phase 3 or M8 profile of a suite program shows it hot; every other new opcode is dispatched through `rare_op` or a second out-of-line arm (`iter_op`, `class_op`, `suspend_op`). Inline from the start: `IterNext` and `IterValue` on the pristine-array path (4.2), the typed-array element paths (4.4), `GetField` on a bound or proxied receiver taking the existing slow path (no new arm). The gate is read in every milestone and a milestone over it moves code out before it merges. If the gate itself proves too tight for the typed-array paths, M6 may raise it to 20 KB with a measured probe table showing no loss on the eight; nothing else may.

### 4.2 Iteration without allocation

`for-of`, array spread, array destructuring, `Array.from`, `Promise.all`, `new Map(iterable)` and `yield*` all run the iterator protocol, and the specification allocates one `{value, done}` object per step. The #122 item "fix iterator-result allocation in `Array.from`-style loops" becomes one mechanism:

- `IterStart a, b` calls `b[@@iterator]()` and stores the iterator in `a` and its `next` method in `a + 1`. If `b` is an `Array` whose shape has no own `@@iterator`, whose prototype chain cell is valid, and the realm's **pristine iteration cell** is valid, no iterator object is allocated: `a` holds the array, `a + 1` holds `Value::EMPTY` as the mark, and `a + 2` holds the index. The cell is a validity cell flipped by any change to `%Array.prototype%[@@iterator]`, `%ArrayIteratorPrototype%.next` or the shape of `%ArrayIteratorPrototype%`, implemented with phase 1's dependents list on both objects.
- `IterNext a, imm` on the marked form reads `elements[index]` (a hole goes through `property::get`), writes the value into `a + 3` and advances, or jumps to `imm` when `index == length` (`length` is read live, as the specification's array iterator does). On the general form it calls `next`, checks `done` and reads `value` through `property::get`.
- `IterClose a` is a no-op on the marked form.
- The natives (`Array.from`, `Promise.all`, the collection constructors, `yield*`'s delegation) use the same two functions from `vm::iter`, so the fast path is not loop-only.

The `ArrayIterator` object still exists for `arr[Symbol.iterator]()` called by user code; only the implicit protocol avoids it. Map and Set iteration takes the general form in phase 4; a kind-specific fast path is an M10 option if a profile shows it.

### 4.3 Suspension: generators and async functions

The register file is one contiguous `Vec<Value>`, so a suspended activation is copied out and in, as V8's interpreter does with its generator register file. A `SuspendedFrame` record holds the frame header (unit, bytecode record, pc, flags, `argc`, the handler depth) and a value-heap range with the registers `base .. top` of the frame at the suspension point. `Yield a, b` (the stack code's `Yield`, which resumes with a value and an is-return flag) copies the window into the record, marks the generator object suspended, and returns `a` to the resumer as the result's value; `next(v)`, `return(v)` and `throw(e)` (natives on `%GeneratorPrototype%`) copy the window back at the current `top`, push a frame from the header, write `v` and the resume kind into the registers the lowering reserved for them, and continue in the nested loop. The compiler's stack code already emits the resume check after each `Yield` (the `return` path runs `finally` blocks through `Gosub`/`Ret`, which phase 2 lowers), so the lowering adds no control flow. `InitialYield` suspends before the first instruction, as the specification's generator start does.

`Await a` is the same suspension with the resumer being a promise reaction job (section 5.4): the frame record is the job's payload, and the job calls the resume entry with the settled value. Async functions return their promise at the first `Await`; async generators queue requests in the object's element range and resume one at a time. `for await` and `yield*` in async generators lower to the iterator opcodes plus `Await` on each result. The frames of a suspended activation are heap records, so the stress mode's missing-root check covers them, and a collection between `Yield` and `next` is the M4 stress test.

Cost: a copy of the live window per suspension and per resume, proportional to the frame's register count, plus one record. No suite program uses generators or `await`; Test262 does. A heap-resident frame that the loop runs in place (QuickJS's model) is an M10 option only if a profile ever shows the copy.

### 4.4 Typed-array element access

`GetElem` and `SetElem` already test the kind before the dense path; `TypedArray` gets the second arm: a canonical index below the view's length reads or writes the buffer's bytes at `byte_offset + index * width` with the kind's conversion (`Int32` and `Uint8` and the two float kinds inline, the other kinds out of line), after a detached and an out-of-bounds check. A write converts with `ToNumber` first, so a user `valueOf` runs before the index is checked, as the specification orders. `Uint8ClampedArray` and `BigInt64Array`/`BigUint64Array` go out of line. Reads from a detached buffer give `undefined`; writes are dropped. The arm reads the view record and the byte arena as heap fields, so it keeps the register window as the cache hits do.

### 4.5 `new.target` and derived constructors

Phase 3 reads `new.target` as the running function of a `CONSTRUCT` frame, which fails for `Reflect.construct(F, args, G)` and for `super()`. A unit that uses `new.target` or is a derived constructor sets a `NEW_TARGET` flag; for such a unit the lowering reserves the first local as the `new.target` register, and `New`, `NewSpread`, `ConstructSuper` and `Reflect.construct` write it at entry. `Frame` stays 32 bytes. In a derived constructor `this` is a cell in the TDZ until `super()` returns (the compiler's `derived_this_local`, which the lowering already treats as a cell for captured bindings), and `ReturnDerived` checks it.

## 5. Language surface

### 5.1 Classes

`DefineClass` lowers to `MakeClass a, b, imm` (constructor unit, parent in `b` or `Value::EMPTY`, the class metadata in the constant table): it creates the constructor function with the derived-class prototype chain (`%Function.prototype%` or the parent, `prototype` inheriting from the parent's `prototype` or `null`), and the lowering emits the method definitions as `DefineMethod`/`DefineGetter`/`DefineSetter` with the home object set on each method's `FunctionRecord` (`PushHomeObject`), `static` members on the constructor. Instance fields compile, in the stack code, into an initializer function the constructor calls (`InstallClassInstanceInitializer`, `CallClassInstanceInitializer`); the lowering keeps that shape and stores the initializer in the constructor's `fields`. Private names are symbols with a `PRIVATE` flag, created once per class evaluation (`InitializePrivateName`), used as ordinary property keys that `OwnPropertyKeys`, proxies and reflection never report; `PrivateIn` and the brand check for private methods test the key's presence. Static blocks and `super` property access (`GetSuperValue`, `PutSuperValue` through the home object's prototype) are out-of-line ops. `CheckCtor` throws when a class constructor is called without `new`.

### 5.2 Destructuring, parameters, literals

Destructuring is iterator and property traffic the stack code already spells out; the lowering declined it for the iterator opcodes (4.2) and for parameter environments. A function with non-simple parameters evaluates its defaults in a scope that closures over them see; with cells for captured bindings a parameter environment is only more cells, created before the defaults run. Rest patterns (`rest_pattern_start`) build an array from the iterator. Object spread (`CopyDataProperties`, with the excluded-keys form for rest patterns) copies own enumerable keys in `OwnPropertyKeys` order through `property::get`. Object-literal methods and computed keys (`DefineMethod`, `DefineMethodComputed`, `DefineFieldComputed`, `SetName`, `SetNameComputed`) are out-of-line ops. Template literals: the cooked and raw string arrays of a tagged template are built once per site and cached in the unit's constant table (the site object is frozen), untagged templates lower to concatenation.

### 5.3 Iterators, `for-of`, generators, async

Section 4.2 and 4.3. The lowering maps `ForOfStart`/`ForOfNext`, `IteratorStart`/`IteratorNext`/`IteratorCall`/`IteratorClose*`, `ForAwaitOfStart`/`Next`, `AsyncIteratorStart`, `Yield`, `YieldStar`, `AsyncYieldStar`, `Await` and `InitialYield` to the new opcodes, and `check_surface` admits the generator and async function kinds. `IteratorClosePreserve` and the `Preserve`/`Detach` forms, which the stack code uses around `finally` blocks, lower to the same close with the pending completion kept in a register.

### 5.4 Promises and jobs

`Runtime.jobs: VecDeque<Job>` with `PromiseReaction`, `PromiseResolveThenable`, `AsyncResume` (4.3), `Finalization` (3.4) and `ModuleEvaluation` (5.6) kinds; a job is a native entry plus rooted values in a value-heap range. Nothing drains the queue implicitly: `Runtime::run_jobs` is called by the host at its boundary (the CLI after the script and after each job; the Test262 worker before it reads `$DONE`), as the current engine and QuickJS do. `HostPromiseRejectionTracker` is a host hook with the CLI's "possibly unhandled rejection" report behind it. `Promise` itself, `then`, `all`, `allSettled`, `any`, `race`, `withResolvers`, `try`, `AggregateError` are ordinary natives over the record.

### 5.5 `eval` and `new Function`

Phase 0: a direct `eval` runs as indirect eval in the global scope. A direct-eval site lowers to an ordinary call of whatever `eval` resolves to at the site, instead of phase 3's `ThrowInternal`; `%eval%` compiles its argument as a script through the compile host (6.3) and runs it on the global object. `Function`, `GeneratorFunction`, `AsyncFunction` and `AsyncGeneratorFunction` compile a function body the same way. `Function.prototype.toString` returns the source text: a unit records its source range, and `Runtime.sources` keeps every compiled source as an append-only store (a snapshot in phase 5 decides whether to keep it).

### 5.6 Modules

A `ModuleRecord` (requested modules, import entries, export entries, status, the environment, the evaluation promise for top-level await) in `Runtime.modules`; its environment is a scope object whose slots are cells, one per exported or module-local binding, with the TDZ through `CheckInit`. An import is a capture: linking resolves each import to the exporter's cell and installs it in the module function's capture list, so `LoadCaptured` reads a live binding with no new opcode; namespace imports create the `ModuleNamespace` kind. Linking and evaluation are the specification's SCC algorithms as the current engine's `modules` implements them, with top-level await through the suspension of 4.3 and the `ModuleEvaluation` job. Dynamic `import()` returns a promise resolved through the host's loader; `import.meta` is an object the host fills. The host contract (`normalize`, `load`, attributes) is the current engine's, behind the `Host` trait, so the Test262 runner's exact-module loader and admissions carry over unchanged. The lowering admits `is_module` units and `Import`, `InitializeModuleImportCollision` and the module-scoped forms of `DeclareGlobal`.

### 5.7 The global lexical environment

Script-level `let`, `const` and `class` move from global-object properties to the `GlobalLexical` dictionary (3.2) with a TDZ, visible across scripts, shadowing the global object, and rejected on redeclaration (`ThrowRedeclaration`). `GetGlobal`/`SetGlobal` probe the lexical dictionary before the global object; the cache entry records which dictionary and its generation, so a hit stays one compare and one load. `DeclareGlobal` performs the specification's `GlobalDeclarationInstantiation` checks.

### 5.8 Coverage

At the end of M7 the lowering declines nothing the current engine's compiler accepts, and `check_surface` is a list of nothing. `with` is a `SyntaxError` in the front end's new-core mode (phase 0), and the compiler is otherwise untouched. The `lowering_census` over the 37 suite programs and over the Test262 corpus (M0 adds the corpus mode) reports zero declines.

**Acceptance (M3, M4, M7).** The two-core corpus gains one program per item: classes (fields, private names and methods, static blocks, accessors, `super` in methods and constructors, derived constructors with and without an explicit return, `new.target` through `Reflect.construct`, `instanceof` with `Symbol.hasInstance`), destructuring (arrays, objects, nested, defaults, rest, holes, iterator close on throw), non-simple parameters with closures over defaults, template literals (tagged, cached site objects, raw strings), spread and literal methods, `for-of` over arrays with and without a modified `@@iterator`, over strings, Maps, Sets, generators and array-likes, generators (`next`/`return`/`throw` at every suspension kind, `finally` on `return`, `yield*`), async functions and `for await`, promises with job order, indirect `eval` and `new Function`, modules (a corpus of fixture graphs with cycles, live bindings, namespaces, TLA and dynamic import), the global lexical environment. Each prints identically on both cores with and without stress; the suspension programs also run under `--gc-stress=1`.

## 6. Builtins

### 6.1 Port order and rules

The current engine's `src/engine/builtins` is 86,769 non-test lines; phase 3 ported about 80 natives (2,466 lines) for the V8 v7 slice. Each row below is ported from the current implementation, keeping its algorithm, its specification step order and its tests at the leaf level, rewritten over section 2's ABI and section 3's kinds. Order is by what blocks the most Test262 variants and what the suites need:

| # | Object | Milestone | Why here |
| --- | --- | --- | --- |
| 1 | `Object` complete (`defineProperty` family, `getOwnPropertyDescriptor(s)`, `create`, `keys`/`values`/`entries`, `assign`, `freeze`/`seal`/`is*`, `getPrototypeOf`/`setPrototypeOf`, `fromEntries`, `groupBy`, `hasOwn`, `__proto__` and the Annex B `__define*__`), `Reflect`, `Symbol` (well-known symbols, `for`/`keyFor`, `description`), `Function` (`bind`, `toString`, `name`/`length`, `Symbol.hasInstance`), `Error` family complete (`cause`, `stack`, `AggregateError`, `isError`), `Proxy`, wrapper constructors, `globalThis` members | M2 | the object model every later builtin uses; `Object` + `Proxy` + `Reflect` + `Function` are 4,400 files |
| 2 | `Array` complete (callbacks, `sort` with QuickJS's merge sort and comparator, `flat`/`flatMap`, `from`/`of`, `copyWithin`, `fill`, `find*`, `includes`, `at`, `with`/`toSorted`/`toReversed`/`toSpliced`, iterators, `Symbol.isConcatSpreadable`; `Symbol.species` dropped per phase 0: the intrinsic constructor is used), the `ArrayIterator` kind and `%ArrayIteratorPrototype%` | M5 | 3,081 files; AWFY and PdfJS sort |
| 3 | `String` complete (`replace`/`replaceAll` with functions, `matchAll`, `normalize`, `localeCompare` as QuickJS's code-unit compare, `padStart`/`padEnd`, `at`, `isWellFormed`/`toWellFormed`, `raw`, `fromCodePoint`, `codePointAt`, the Annex B HTML methods and `substr`), the `StringIterator` kind | M5 | 1,223 files; ropes land with it |
| 4 | `RegExp` complete (the `Symbol.match`/`replace`/`search`/`split`/`matchAll` protocol, `exec` result with `indices` and `groups`, flags accessors, `lastIndex` semantics for `g`/`y`, `RegExpStringIterator`, `compile`, `escape`) over the shared `oxide-regexp` | M5 | 1,879 files; the Octane RegExp program |
| 5 | `Number`, `Boolean`, `Math` complete (`toFixed`/`toExponential`/`toPrecision` from the current `primitive` port, `imul`/`fround`/`clz32`/`hypot`/`sumPrecise`…), `BigInt` (`asIntN`/`asUintN`, `toString(radix)`), `parseInt`/`parseFloat` done, URI functions, `escape`/`unescape` | M5 | `zlib` needs `imul`; the formatting functions are the harness's `toFixed` |
| 6 | `JSON` (`parse` with reviver, `stringify` with replacer, gap, `toJSON`, `rawJSON`/`isRawJSON`), `Date` complete (parsing, the calendar, local time through the host, `toLocale*` as QuickJS's fixed formats) | M5 | 165 + 594 files; Date needs the clock and time zone of 6.3 |
| 7 | `Promise` and the async intrinsics (`%AsyncFromSyncIteratorPrototype%`, async generator prototype, `%AsyncIteratorPrototype%`) | M4 | with the suspension of 4.3, since the harness's `asyncHelpers.js` needs them |
| 8 | `Iterator` and the helpers (`map`, `filter`, `take`, `drop`, `flatMap`, `reduce`, `toArray`, `forEach`, `some`, `every`, `find`, `from`, `concat`, `Iterator.prototype[@@iterator]`), `IteratorHelper` and `IteratorWrap` kinds; `%GeneratorPrototype%`'s methods | M4 | 510 files; they are natives over 4.2's protocol functions |
| 9 | `ArrayBuffer` (resizable, `transfer`, `detached`), `SharedArrayBuffer` (growable), `%TypedArray%` and the twelve constructors (`set`, `subarray`, `slice`, the callback methods, `sort`, `toSorted`, `from`/`of`, `Uint8Array` base64 and hex codecs), `DataView`, `Atomics` (`wait`/`notify` on shared memory with the agent host) | M6 | 3,300 files; PdfJS, Mandreel, GameBoy, zlib |
| 10 | `Map`, `Set` (with the set-algebra methods `union`, `intersection`, …, and `Map.groupBy`), `WeakMap`, `WeakSet`, `WeakRef`, `FinalizationRegistry` | M6 | 813 files; Havlak's `Set` |
| 11 | `eval`, `Function` constructors, `Function.prototype.toString` source text, modules and `import()` | M7 | needs the compile host |
| 12 | `$262` (`evalScript`, `detachArrayBuffer`, `gc`, `global`, `IsHTMLDDA`, `agent`; `createRealm` throws `TypeError: realms are not supported`), qjs's `print`, `console.log`, `scriptArgs`, `load`, `read` | M0 (host), M6 (agent) | the Test262 runner and the suite shims |

Rules for every native, in addition to phase 3's: a conversion that may run JavaScript happens where the specification orders it, and the old implementation's tests pin that order; a native that observes the length of an array-like reads it through `property::get` unless the receiver is a dense `Array` with a valid chain cell, in which case it reads the element range directly (the fast path every `Array.prototype` method takes on plain arrays); natives never allocate an iterator result for the implicit protocol (4.2); a native that needs the current time, the time zone, a module or compiled code asks the `Host` (6.3) and never the platform directly, so `oxide-core` stays dependency-free and the PSP build of phase 5 supplies its own host.

### 6.2 `Symbol.species` and the other drops in the port

Where the current implementation reads `constructor[@@species]` (`Array` methods, `%TypedArray%` methods, `Promise` combinators, `RegExp` protocol methods, `ArrayBuffer.prototype.slice`), the port constructs the intrinsic directly. `Symbol.species` and `Symbol.unscopables` exist as symbols with no reader. `createRealm` throws. The port of each row records the Test262 variants that fail for exactly these reasons, and section 7.2's frozen list is checked against that record at M9.

### 6.3 The host boundary

`oxide_core::host::Host` is a trait object installed at runtime creation:

```rust
pub trait Host {
    fn write(&mut self, stream: Stream, text: &str);
    fn now_ms(&mut self) -> f64;
    fn local_tz_offset_minutes(&mut self, utc_ms: f64) -> i32;
    fn compile(&mut self, request: CompileRequest<'_>) -> Result<CompiledUnits, CompileError>;   // scripts, eval, Function, modules
    fn normalize_module(&mut self, referrer: &str, specifier: &str, attributes: &[(String, String)]) -> Result<String, HostError>;
    fn load_module(&mut self, name: &str, attributes: &[(String, String)]) -> Result<ModuleSource, HostError>;
    fn promise_rejection(&mut self, event: RejectionEvent, promise: Value);
    fn import_meta(&mut self, module: &str) -> Vec<(String, Value)>;
}
```

`CompiledUnits` is what `lower_script` and the module lowering already produce: published code units plus the tree `Runtime::instantiate` takes. The root crate's `api::new_core` implements the trait with the front end, the lowering and the `adapters` host services; the Test262 runner wraps it with the exact-module loader; `oxide-run` gets the system clock and the local time zone from the same adapter the current CLI uses. A host method may allocate through the runtime it is handed (`compile` publishes units), but never holds a borrow across the call: the trait takes `&mut Runtime` as the request's parameter, not `&mut Heap`.

**Acceptance (M2, M5, M6).** Per row: one corpus program exercising every listed member with the current `builtins` tests' leaf-level edge cases, printing identically on both cores with and without stress; the row's Test262 area passes on the new core except the variants recorded under 6.2; `check-new-core-stress.py` green. M6 additionally: the four typed-array Octane programs print their completion lines, and the `$262.agent` tests of `Atomics` pass with the threaded agent host.

## 7. Test262 on the new core

### 7.1 The runner's second backend

`conformance/test262` runs each variant in an isolated worker process over the current engine's `Runtime`/`Context` API. It gains `--engine new-core` (and the `TEST262_ENGINE` environment variable the gate script forwards): the worker runs the harness files and the test through `api::new_core` with the `Host` of 6.3, the `$262` object of row 12, the exact-module loader, the job drain before `$DONE` is read, and the same metadata, admission, negative-diagnostic and capability logic as today. Nothing in the coordinator, the admissions or the report format changes; the result rows are the same shape, so `verify-report.cjs` and the frozen-receipt comparison work unchanged. The current engine's backend is the default and its frozen receipts in `current.conf` are not touched; this pull request runs the current engine's focused and full gates because it edits the runner.

A second spec file, `dev-support/test262/new-core.conf`, pins the new core's profile, receipts and hashes with the same keys as `current.conf` plus `engine=new-core`; `test-test262.sh --spec dev-support/test262/new-core.conf --focused|--full` is the entry point, and the engine fingerprint already covers `core/` and `regexp/`.

### 7.2 The frozen list of dropped variants

Phase 0 §2 promised "the exact variant list is produced by one full Test262 run of the current engine with the five behaviors switched off, and is frozen next to the Test262 admission data"; it was not produced. M0 produces it: the current engine gains a Cargo feature `dropped-features-probe` (never in a release, never in the default gates) that makes `with` a `SyntaxError`, runs direct `eval` as indirect, makes `$262.createRealm` throw, skips the `@@species` lookups and ignores `@@unscopables`; one full run with the feature yields the set of variants that pass today and fail under it, frozen as `dev-support/test262/new-core-dropped-variants.tsv` (path, variant, reason) with its count `D` and sha256 in `new-core.conf`. Phase 0's static estimate was 1,738–3,197 variants; the measured `D` replaces it, and the exit condition of section 10 is `83,304 − D` with the list as the only admitted difference. Variants the current engine fails (the 50 known failures) stay failures either way.

### 7.3 What the harness needs first

The harness files every test includes (`assert.js`, `sta.js`, `doneprintHandle.js`) use only phase 3's surface; `propertyHelper.js` needs `Object.getOwnPropertyDescriptor`, `Object.defineProperty` and `Array.prototype.join`/`map`; `compareArray.js` needs `Array.isArray` and `join`; `asyncHelpers.js` needs async functions and `Promise`; `testTypedArray.js` and friends need the constructors of row 9. The M0 baseline run therefore reports most variants as runtime failures, and the per-milestone counts of section 10 rise as the rows land; the count after M2 is the first meaningful one.

## 8. The driver and CI

`oxide-run` grows into the qjs surface the suites and Test262 use: `-e`, `--module`, `--std` for `load`/`read`, `scriptArgs`, `console.log`, the job drain after the script, `--engine` is not needed because `oxide-run` is the new core. `--gc-stress`, `--ic-stats`, `--gc-stats`, `--heap-cap-mib` and `--disassemble` stay. `lowering_census` gains `--test262 DIR` to count declines over the corpus.

CI:
- `fast` job: as today plus, from M9, the focused vector on the new core (`new-core.conf --focused`), which takes about the time the current focused run takes.
- `new-core-suite` tier (nightly and dispatch): the 37 programs at one `run()` under stress, with the per-program interval table extended from Splay's 100 to the typed-array programs as M8 measures them; stdout compared to the one-run manifest.
- A new nightly `new-core-test262-full` dispatch tier: `new-core.conf --full`, receipts compared.
- The per-pull-request instruction gate of section 10 is a reference-machine measurement recorded in the pull request, not a CI job (CI machines do not count instructions reproducibly).

## 9. Milestones

| # | Content | Acceptance | Estimate |
| --- | --- | --- | --- |
| M0 | the runner's second backend (7.1) and `new-core.conf`; the `$262` host and qjs helpers of row 12; the dropped-variant list (7.2); the census over Test262; the loop-size and instruction gates wired into the pull-request template | a full Test262 run on the new core produces a receipt; `D` frozen; the current engine's gates unchanged | 4–6 days |
| M1 | performance margin: the call sequence (`call0`) and element writes (`array_write`) read from the assembly and the phase 3 profiles and fixed; the `rare_op`/`iter_op`/`class_op`/`suspend_op` split prepared | `call0` and `array_write` slopes below 1.3× QuickJS; every V8 v7 program's instructions at or below the phase 3 M6 count; `vm::run` under the gate | 4–6 days |
| M2 | the internal methods by kind (3.1), the kinds of 3.2 except typed arrays, collections, promises and modules; row 1 of 6.1; function `name`/`length`; the global lexical environment (5.7); `new.target` (4.5) | row 1's corpus and Test262 areas; `language/global-code`, `block-scope`, `identifiers`, `types` | 12–16 days |
| M3 | classes (5.1), destructuring, parameters, literals and templates (5.2), the iterator opcodes and the pristine-array path (4.2), `for-of` | section 5.8's corpus for these items; Test262 `class`, `object`, `for-of`, `assignment`, `destructuring`, `arrow-function`, `template-literal` | 14–18 days |
| M4 | suspension (4.3), the job queue and promises (5.4), async functions and generators, `for await`, rows 7 and 8 | the suspension corpus under `--gc-stress=1`; Test262 `generators`, `async-*`, `for-await-of`, `Promise`, `Iterator`, `GeneratorPrototype` | 12–16 days |
| M5 | rows 2–6: `Array`, `String` with ropes (3.3), `RegExp`, `Number`/`Boolean`/`Math`/`BigInt`, `JSON`, `Date` with the host clock | each row's corpus and Test262 area; TypeScript and Code-Load print their completion lines; the string-work share of TypeScript and PdfJS recorded before and after ropes | 16–22 days |
| M6 | rows 9 and 10: buffers, typed arrays with the element fast path (4.4), `DataView`, `Atomics` with the threaded agent host, collections with ephemeron marking (3.4), weak references | the four typed-array Octane programs and Havlak print their completion lines; Test262 `TypedArray*`, `ArrayBuffer`, `SharedArrayBuffer`, `DataView`, `Atomics`, `Map`, `Set`, `Weak*`, `FinalizationRegistry` | 14–18 days |
| M7 | the compile host (6.3), `eval` and the `Function` constructors (5.5), modules and dynamic import (5.6), `Function.prototype.toString` | the module corpus; Test262 `module-code`, `import`, `dynamic-import`, `eval-code` outside `D`, `eval`, `Function`; `check_surface` admits everything | 10–14 days |
| M8 | the 37 programs on `oxide-run`: shims, declines at zero, fallout fixes; the nightly tier over all 37 with per-program stress intervals; a first indicative timing | every suite program prints the manifest's stdout; `check-new-core-suite.py` green over 37 | 6–8 days |
| M9 | Test262 convergence: triage the full run by area, fix to the exit number; the focused vector on the new core in `fast`; `D` checked against the per-row records of 6.2 | section 10's Test262 condition holds; the new core's full receipt frozen in `new-core.conf` | 12–20 days |
| M10 | profile and tune on the 37 (the decisions of section 11 assigned here); the reference-machine measurement; the phase 4 results page with Tables A–E and the floor read | section 10's tables and the decision committed under `docs/performance` | 8–12 days |

Total: about 110–155 working days. M2 and M3 may split into two pull requests each; M5 is always one pull request per row; M0's runner change and probe feature are two pull requests, each Test262-gated on the current engine. M1 precedes M2 because every later milestone is measured against the margin it creates.

## 10. Measurement, exit and the floor

Every number is produced as in phases 1–3: the [baseline](baseline.md)'s machine, binaries and manifest, `fixed.py --repeat 5 --cpu 2 --linux-counters --timeout 900`, the three engines interleaved in rotating order, medians of 5, every sample's stdout checked, raw samples kept outside the repository; probe slopes from `probes/new-core/phase2.py`.

**The per-pull-request instruction gate.** Every milestone's pull request records, on the reference machine, `instructions:u` of the eight V8 v7 programs at the full fixed work (one run each, pinned) against the phase 3 M6 binary (`a25e812d`, sha256 `b6644338628cbed9…`). A program that rises by more than 2% is explained in the pull request by a profile or fixed before it merges; the loop-size check accompanies it. This is the mechanism that keeps NavierStokes above the floor through a phase that adds opcodes.

**Table A: the 37 suite programs.** Whole-process time and instructions for QuickJS, the current core and the new core; the two ratios per program; the geometric means per suite (V8 v7 eight, Octane 15, AWFY 14). The gate is read from this table.

**Table B: the fixed probes and the phase 1 workloads**, as phase 3's Table B, with `call0` and `array_write` beside their M1 values; and the new probes this phase adds to the fixed matrix: `for_of_array` (a `for-of` loop over a dense array), `spread_call`, `class_method` (a method call through a class prototype), `generator_next`, `typed_write` (`Float64Array` fill), `map_get`, `string_build` (`+=` loop of 10⁵ pieces), `sort_cmp` (`sort` with a comparator), each with the QuickJS slope.

**Table C: the loop and the caches.** `vm::run`'s size against the gate; `ic-stats` per program for all 37; the pristine-iteration cell's hit count on the programs that iterate.

**Table D: the collector on the 37**, as phase 3's Table D, at the default trigger. If any program other than Splay is above 10% of its time in the collector, M10 records the growth-3 and growth-4 numbers for it as phase 3 M5 did for Splay, and the decision stays the maintainer's.

**Table E: Test262.** The new core's full run: QuickJS-scope passes, the full-pass metric, and the list of variants that pass on the current engine and fail on the new core, which must be a subset of the frozen `D`.

**Exit condition.** Tables A–E committed under `docs/performance`; `gc-stress` green on everything in section 8; the current engine's Test262 unchanged (focused 6,844 of 6,844; full 83,304 of 83,354). Then:

- **Test262.** Every variant that passes on the current engine and is not in `D` passes on the new core: QuickJS-scope passes ≥ 83,304 − `D`, and Table E's difference list is empty outside `D`. The focused vector holds the same way.
- **The suites report.** All 37 programs print the manifest's stdout on `oxide-run` in Table A.
- **The floor, raised to 3.0.** Over the eight V8 v7 programs, the geometric mean of (current-core time / new-core time) is at least 3.0 and no program other than RegExp is below 3.0 (the maintainer's decision of 2026-10-17, replacing phase 3's 2.5). This page extends the same floor to Octane and AWFY: the geometric mean per suite is at least 3.0, and every program is at least 3.0 except the two whose time is in components both cores share, `octane-regexp` (the regular-expression engine) and `octane-code-load` (the front end: its work is parsing and compiling), which are reported with the share of their time spent in the shared component. The maintainer confirms or changes the extension before M10 runs.
- **No regression against QuickJS.** The new core's geometric mean over the eight is reported against phase 3's 0.93× QuickJS; it is a report, not a gate.

Failing the floor on a suite program is not a no-go for the phase in #122's sense (the architecture was decided at phase 3's gate); it is a finding that names the mechanism, recorded in the results page, and M10 does not end until either the floor holds or the maintainer accepts the exception in writing on that page.

## 11. Decisions

Made, and not reopened inside phase 4:

- natives are straight-line Rust over the re-entrant `call` with root-stack rooting; the Step/Resume form is not ported (section 2);
- generators and async functions copy their register window out and in through a heap record; the loop runs no heap-resident frames (4.3);
- the implicit iterator protocol over an unmodified `Array` allocates no iterator and no result object, guarded by one validity cell (4.2);
- internal methods dispatch on `ObjectKind` in the slow paths only; the inline cache hit paths never see an exotic receiver (3.1);
- ropes from 64 code units, flattened in place without recursion (3.3);
- `Symbol.species`, unscopables, Realms, local `eval` and `with` stay dropped; `createRealm` throws (6.2);
- the front end stays in the root crate and reaches the core through the `Host` trait; `oxide-core` gains no dependency (6.3);
- `#![forbid(unsafe_code)]` stays in every workspace crate. #122's optional `get_unchecked` helper is formally dropped: phase 3's exit measured the new core below QuickJS's time without it, and the README's "fast without `unsafe`" is a property the project keeps. `SharedArrayBuffer` and the agents use `std::sync::atomic` and `Arc`, as the current engine does;
- the collector's trigger stays at a 4 MiB minimum and growth 2 (phase 3 M5's measurement: growth 3 buys Splay 100 ms for about 170 MB more reserved heap); the lever for Splay is phase 5's nursery, and the maintainer may change the factor at any time by editing one constant and re-running Table D;
- the public embedding API is phase 6 work (section 13);
- the floor is 3.0 per program and per geometric mean, with the two shared-component exclusions of section 10;
- the loop-size gate is 16 KB, with the one conditional raise of 4.1.

Open, with the milestone that decides them:

- a kind-specific `IterNext` for Map and Set iterators: M10, by the profile of Havlak and any Test262-heavy program;
- a heap-resident generator frame run in place instead of the copy: M10, only if `generator_next`'s slope is above 1.5× QuickJS;
- whether `Array.prototype` methods on dense arrays take the element range directly past the first `ToNumber` callback (the fast path's exact admission): M5, by the `sort_cmp` and `for_of_array` slopes;
- whether `%TypedArray%` callback methods inline the element read per kind or convert through one generic reader: M6, by the Octane typed-array profiles;
- the per-program stress intervals of the nightly tier for the typed-array programs: M8, by their run time at interval 1;
- whether the string-work share of TypeScript and PdfJS justifies a two-byte-aware rope flattening into a preallocated buffer, or the simple per-flatten copy suffices: M5, by the before/after numbers;
- raising the loop-size gate to 20 KB for the typed-array arms: M6, by Table B on the eight;
- the growth factor for programs other than Splay that Table D shows above 10%: M10, maintainer's call.

## 12. Risks

- **The floor through loop growth.** NavierStokes sits 1.6% above 3.0 and `vm::run` grows in every milestone. Mitigation: M1 creates margin before anything is added; new opcodes default to out-of-line arms; the per-pull-request instruction gate catches a 2% rise where it happens, not at M10.
- **Rooting in 87k ported lines.** Every native holds values across calls. Mitigation: the ABI's `Rooted` helper makes the rooted form the easy form; every row's corpus runs under `--gc-stress=1` before its Test262 area is counted; the stress verifier's poisoning turns a missed root into a panic at the use.
- **Native-stack depth.** Deep recursion through `forEach`, `sort` comparators, getters and proxies grows the Rust stack per level. Mitigation: the 1 MiB budget already throws `InternalError: stack overflow`; M2 adds a test that recurses through each native kind to the limit and checks the error, and the CLI's thread stack size (`declare_thread_stack`) is set as the current CLI sets it.
- **Suspension and the register file.** A frame resumed at a different `top` than it was suspended at must have every absolute register index in its header rebased (the caller's destination register is absolute). Mitigation: the header stores the destination relative to `base`; the M4 corpus resumes generators from different call depths and under stress.
- **Ephemerons and the stress verifier.** Weak tables break the verifier's "live equals reachable" check unless it understands them. Mitigation: the verifier learns ephemeron semantics in the same pull request as the marking (M6), with a test that a value reachable only through an unmarked key is freed.
- **Proxies in the slow paths.** A proxy can appear as a prototype, as a `Function.prototype`, as an iterator, as `arguments`' callee. Mitigation: every `property::` entry point dispatches first (3.1); the Proxy Test262 area is in M2, before any builtin is ported, so the builtins are written against it.
- **Modules and the runner's admissions.** The exact-module admissions (`admissions.tsv`) encode graph shapes the current loader satisfies. Mitigation: the loader contract is kept verbatim behind the `Host` trait, and M7's acceptance is the admitted module cohort, not a hand-written corpus alone.
- **Agents.** `$262.agent` runs scripts on threads with shared memory and a broadcast protocol. Mitigation: a runtime per agent thread, the shared bytes behind `Arc<[AtomicU8]>`, and the current `test262_agent.rs` protocol ported as is.
- **The dropped-variant list.** If the probe feature of 7.2 drifts from the new core's actual behavior, `D` is wrong in one direction or the other. Mitigation: M9 reconciles `D` against the per-row records of 6.2; every variant in the difference list is classified by hand, and the results page records the reconciliation.
- **Date and the time zone.** Test262's Date tests depend on the local zone through the host. Mitigation: the Test262 worker fixes `TZ` as the current gate does; the host adapter is the current CLI's.
- **Estimate.** This is the largest phase, and M9's duration is fallout the plan cannot size. Mitigation: the per-milestone Test262 counts (7.3) are the progress signal, reported in every milestone's pull request so a slip is visible by M5.

## 13. Seam with phases 5 and 6 (preview, not phase 4 work)

Phase 5 takes the `Host` trait as the embedding boundary the PSP build implements; `Runtime.sources`, the compiled regular-expression store, the code store, the module records and the `IcStore` are the append-only stores a snapshot serializes by index, and nothing in phase 4 introduces a pointer into them. The nursery, if measurements call for it, moves young objects: the kinds of 3.2 keep every cross-object reference as an index in a header, a raw slot or a payload record, which is the precondition for a rewriting copy.

Phase 6 re-targets `engine::api` (`Runtime`, `Context`, `Value`, handles, the host services, module loading, jobs, profiling) at the new core over the same `Host` trait, switches the CLI's `qjs` and the adapters, then deletes the old core; with phase 4's drivers and host in place that work is mechanical but not small, and #122's size S for phase 6 should be re-estimated as M when phase 5 is planned.
