# oxide-core

The new core of quickjs-oxide (issue #122), built in phases next to the current
engine. Phase 1 is the value and heap layer; phase 2 adds register bytecode
and its main loop. As of phase 2 M4 the instruction set, code units, their
validator, the lowering from the current compiler's stack code
(`src/engine/compiler/register` in the root crate) and the loop that runs it
exist; the only natives are `print` and the error constructors. The CLI
crate's `oxide-run` binary runs a script file on it (`oxide-run --help` for
the options). The plan is [`docs/performance/new-core-phase1.md`](../docs/performance/new-core-phase1.md);
the measurements are in [`docs/performance/new-core-phase1-results.md`](../docs/performance/new-core-phase1-results.md);
phase 2 is planned in [`docs/performance/new-core-phase2.md`](../docs/performance/new-core-phase2.md)
and measured in [`docs/performance/new-core-phase2-results.md`](../docs/performance/new-core-phase2-results.md);
phase 3 (inline caches, the V8 v7 slice and the go/no-go) is planned in
[`docs/performance/new-core-phase3.md`](../docs/performance/new-core-phase3.md)
and measured in [`docs/performance/new-core-phase3-results.md`](../docs/performance/new-core-phase3-results.md);
phase 4 (the builtins, strings and the full language surface) is planned in
[`docs/performance/new-core-phase4.md`](../docs/performance/new-core-phase4.md)
and measured in [`docs/performance/new-core-phase4-results.md`](../docs/performance/new-core-phase4-results.md).
As of phase 4 the core runs every construct the current compiler accepts
except the five phase 0 dropped, all 37 suite programs, and Test262. Since
phase 6 M2 it is the engine `run-test262` and the web playground run, through
the embedding API `api::core` ([phase 6](../docs/performance/new-core-phase6.md)).

The crate has `#![forbid(unsafe_code)]` and one dependency, the
regular-expression crate `oxide-regexp` (`regexp/`, phase 3 section 6.1),
which has none. From phase 2 the root crate depends on it (the register
lowering lives there); never the reverse.

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
| `vm` | the register file and frame table, the main loop (`interp`), conversions and operators (`ops`, `number`, `bigint`), property access (`property`) and the internal methods by kind (`object`, `proxy`, `typed`, `arguments`), iteration (`iter`), suspension (`suspend`, `async_generator`), jobs and promises (`jobs`), classes (`class`), modules (`module`), the compile host (`compile`), errors and the native ABI (`error`, `native`) |
| `builtins` | the realm's constructors, prototypes and globals as natives over the re-entrant `call` (phase 4, section 2), one file per builtin object, plus `$262` (`test262`, `agent`) |
| `host` | the `Host` trait: output, clock and time zone (phase 4, section 6.3) |
| `runtime` | `Runtime`: the heap, the code store beside it, the single realm and the host services |

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
- Bindings captured by an inner function live in *cells* (one-slot scope
  objects) rather than in one scope object per activation and per block
  (plan section 3.1). The lowering consumes the compiler's finished stack code,
  where block structure is already expanded into QuickJS's `close_loc`
  discipline; cells reproduce that discipline exactly. The binding's register
  holds its cell (`NewCell`, `LoadCell`, `StoreCell`), `CloseLocal` becomes
  `CloseCell` (a fresh cell holding the current value: a `for (let …)`
  iteration), and a closure holds one cell per captured binding, listed by the
  child unit's `captures` and read with `LoadCaptured`/`StoreCaptured`. The
  scope-chain opcodes (`LoadScope`, `EnterScope`, …) are gone, and so is the
  frame's scope field: a function reaches its captures through its function
  object. No compiler change is needed to mark captured bindings.
- Calls follow the stack code's operand order rather than reserving a receiver
  slot: `Call a, b, c` has the callee in `b` and the arguments from `b + 1`,
  so the callee's window starts at `b` (its `this`, written by the call);
  `CallMethod` and `New` have receiver or callee and new.target in `b`,
  `b + 1`, and the window starts at `b + 1`. No argument moves either way.
- `Swap` exchanges two registers, for cyclic stack shuffles; `DeclareGlobal`
  instantiates a script's `var` and function declarations.
- Script-level `let` and `const` are global object properties without a TDZ
  in phase 2; the global lexical environment comes with the global IC.
- Functions with more than 256 registers were reported unsupported by the
  lowering in phase 2; phase 3 M3 gives their locals a spill scope (below),
  and phase 6 M4b their temporaries past the window too.

### The loop (M3, M4)

- A `Frame` is 32 bytes, not 24: it keeps the running function's bytecode
  record next to its unit, and the caller's destination as an absolute
  register index. There is no scope field (captures are cells, above).
- Entering a frame clears the window past the arguments to `undefined`:
  locals need it anyway, and a register above a returned frame's `top` may
  name a record freed since, which must not be marked once the window covers
  it again. Temporaries below `top` are still marked conservatively.
- The register file holds at most `DEFAULT_STACK_LIMIT` (2^20) values;
  a call past it throws `InternalError: stack overflow`, QuickJS's error,
  rather than the plan's `RangeError`.
- A conversion may call back into JavaScript (`valueOf`, `toString`) through
  a nested loop on the frames above its own; such re-entries share a 1 MiB
  native-stack budget measured from stack addresses, as QuickJS's
  `js_check_stack_overflow` does. Natives stay leaf functions.
- Without builtins, `ToPrimitive` stands in for the builtin a missing
  `toString` lookup would have found: `Object.prototype.toString`,
  `Array.prototype.toString` (a `,` join) or `Error.prototype.toString`. The
  realm defines the error constructors and prototypes, `globalThis`,
  `undefined`, `NaN` and `Infinity`; `Runtime::install_print` adds `print`.
  Functions have no `name` or `length`; a constructor gets its `prototype`
  object at `MakeClosure`. Property reads on primitives other than a
  string's `length` and indices go to `%Object.prototype%` (from phase 3 M1,
  a string's go to `%String.prototype%`).
- Index keys live in the element range while an index stays within four
  times its capacity; a farther index is a named property with an index key,
  and reads check the elements first.
- `TailCall` does not reuse the frame: the callee's frame is marked so its
  result is also its caller's. The compiler emits no tail call.
- A thrown value travels as an unrooted `Throw`; the unwinder stores it in
  the handler's register (or returns it to the embedder) before anything
  allocates.

## Phase 3 departures

### Inline caches (M1)

- The `IcStore` lives inside the `Heap`, beside the register file, rather
  than as a third field of `Runtime`: collection happens inside heap
  safepoints, and the store must be pruned after every marking before an
  index is recycled. The loop still reads it as `cx.heap.ics`, and the
  code store stays outside the heap.
- Cache sites are numbered by `CodeBuilder::finish`, so units built by hand
  and lowered units are numbered alike: the named-property sites first in
  instruction order (0 to 254 in `c`, the rest sharing the reserved site
  255), then the global forms (`b | c << 8`), then `New` (`imm`).
  `CodeStore::publish` assigns `ic_base` and `Runtime::publish` reserves the
  entries, with each site's kind for pruning.
- The `GetField` hit reads the object, cell and value arenas as fields of
  the heap, disjoint from the register file, so the loop keeps its register
  window across a hit instead of retaking it.
- The realm gains an empty `%String.prototype%` (inheriting from
  `%Object.prototype%`); M4 fills it. A string-receiver entry is guarded by
  the cell of `%String.prototype%`'s own chain (`Heap::chain_cell`), which
  any change to it or to `%Object.prototype%` clears.

### Stores, globals, constructors, accessors (M2)

- The global generation lives in the `Heap` (`global_generation`), not in
  the global object's `Dictionary`: every insert, remove and reflag of the
  global object's dictionary bumps it, so a global hit is one load and one
  compare against the entry. On wrapping to the empty entry's tag the
  generation restarts at 0 and every global entry is cleared.
- A `SetField` transition hit also requires the object to be extensible and
  not used as a prototype: extensibility and prototype use are object flags,
  not shape properties, and a prototype's shape change must invalidate.
  An in-place entry carries the static cell; only transitions consult the
  chain's cell, which the fill takes only when no prototype has the key as
  an accessor or a read-only property (decided before the write runs, since
  a setter may add the key itself).
- A `New` site learns its receiver size at the constructor's return from the
  `New` instruction before the caller's resume point, so `Frame` carries no
  site. The learned size is the receiver shape's slot width, capped at
  twice the dictionary threshold.
- Shapes count slots (`width`) apart from properties (`count`): an accessor
  is two slots. A reflag never changes a property between data and
  accessor; the literal paths delete and re-add instead.
- `DefineGetter` and `DefineSetter` are core opcodes from M2, and the
  lowering accepts `get`/`set` in object literals from M2 rather than M3, so
  the two-core corpus covers accessors (`core/tests/corpus/accessors.js`).
- The `ic-stats` counters exist with the feature and in debug builds, so
  tests see hits; release builds without the feature carry none.
  `Runtime::ic_report` prints them, and `oxide-run --ic-stats` (CLI feature
  `ic-stats`) prints the report to stderr at exit.

### The language surface (M3)

- A mapped `arguments` object keeps its parameters' cells in a scope object
  named by its `payload` (slot 0 the mapped count, then one cell per mapped
  parameter, `Value::EMPTY` once unmapped) rather than in a new payload
  arena. An unmapped one's payload is `NONE`.
- A function that reads its actual arguments (the `ARGUMENTS` unit flag,
  for `arguments` and rest parameters) keeps the ones past its parameters
  above its registers when it is entered; `Frame` carries `argc` in its
  padding and stays 32 bytes. `new.target` needs no frame slot: it is the
  running function in a `CONSTRUCT` frame, since phase 3 has no
  `Reflect.construct` or `super`.
- Captured parameters are cells, so `NewArguments` and `Rest` read a
  parameter register through its cell when it holds one: cells are scope
  objects, which are never JavaScript values.
- `Function.prototype.call` and `apply` are natives that the `Call` arm
  rewrites (an out-of-line `rewrite_call`) into a call of their receiver;
  natives now receive `this` through `CallSite`.
- `ForInNext` jumps to the loop body with a key and falls through when the
  enumeration is exhausted, the shape of the stack code's `ForInNext` and
  its `IfFalse`. The enumeration is a `ForInIterator` object: the key
  snapshot in its elements (index keys as `Int32`, turned into strings as
  they are yielded), the object and the count in two raw slots, the cursor
  in its payload.
- Regular-expression literals and `new RegExp` share one append-only store
  keyed by the `(source, flags)` atoms, beside the code store in `Runtime`
  and reached from the loop through `Cx`; M4 compiles the programs.
- A direct `eval` lowers: the call throws `InternalError: unsupported: eval`
  (`ThrowInternal`), so the function around it has no dynamic bindings, and
  `HasDynamicBinding` is `false`. Earley-Boyer's never-called `sc_jsNew` is
  the one such function in the V8 v7 programs.
- A function whose locals do not fit the 256-register window keeps the rest
  in the slots of a spill scope (`NewSpill`, `LoadSlot`, `StoreSlot`, and
  `Capture::Slot` for a child capturing one), held in the register after the
  window's locals, with the temporaries above it. Locals placed past the
  temporaries would be overwritten by every call, whose window starts at a
  temporary. RegExp's `RegExpBenchmark` (300 locals) is the one such
  function in the suite. A function whose operand stack does not fit
  either (phase 6 M4b) keeps its deeper temporaries in slots of the same
  scope, reached through eight scratch registers above the ones that fit;
  a call that does not fit in the scratch registers becomes `CallSpread`
  over an array of its arguments.
- The register file's dead registers above a returned frame are cleared
  when a frame resumes over them if a collection has run since they were
  last above `top` (`VmStack::stale_from`). Phase 2 cleared only a callee's
  own window, so a caller's dead temporaries above a smaller callee could
  name freed records when they became roots again; the M3 corpus under
  stress found it.
- The opcodes no hot path runs (accessors in literals, the spill scope,
  `arguments`, rest, `new.target`, regular-expression literals, `for-in`
  setup, spread elements, unsupported constructs) share one loop arm that
  calls an out-of-line `rare_op`, which keeps `vm::run` under its size gate.

### The realm (M4)

- Natives are one `Native` enum generated with its table of entry points and
  constructor flags (`builtins::natives!`), indexed from the function record;
  `vm::native` keeps only `CallSite` (receiver, arguments, and whether `new`
  called) and the dispatch, which cuts the root stack back after every
  native so natives root intermediate values on it freely.
- `%Function.prototype%` is a native function returning `undefined`;
  `%Number.prototype%` and `%Boolean.prototype%` join the intrinsics, and
  number and boolean receivers read through them.
- Errors carry an `ERROR` object flag set at creation, read by
  `Object.prototype.toString`.
- Phase 2's `ToPrimitive` stand-ins are gone: `toString` and `valueOf` are
  the builtins.
- `Number.prototype.toString(radix)`, `parseInt` and `parseFloat` are ports
  of the current engine's exact algorithms onto the core's own `Big`
  (`vm::radix`); radix-10 formatting of non-integers uses them too, since
  Rust's shortest formatting differs from QuickJS on ties and on some powers
  of two.
- Regular expressions compile when a literal is first evaluated or
  `RegExp` is called, into the program store, and run on a string's own
  one-byte or two-byte storage. `split` with a regular expression searches
  forward from each position instead of running the specification's sticky
  copy, which finds the same matches and leaves the user's `lastIndex`
  alone.
- Function objects have no `name` or `length`, and `Function.prototype.toString`
  prints every function as QuickJS prints a native one (source text is
  phase 4's). `new String`, `new Number`, `new Boolean`, primitive
  wrappers, `new Function` and a function replacement in `replace` throw
  `InternalError: unsupported: …`.

### Tuning (M5)

- `GetField` has a `length` entry (`LENGTH_TAG`) that reads an array's
  stored length or a string's unit count.
- A prototype hit on a dictionary-mode holder is cached: any change to a
  prototype clears the cell, dictionary or not. A dictionary-mode receiver
  is still never cached.
- A megamorphic `GetField` site probes a shared table of 4,096 entries,
  keyed by receiver shape and property key, and fills it from its misses.
- Constructed receivers and empty object literals reserve at least four
  slots; a constructor site learns sizes only past that.
- `Mul` on doubles keeps a double result, as `Add` and `Sub` do. `Div`
  still gives an `Int32` when the quotient is one.
- Radix-10 formatting of a non-integer walks down from 17 digits with
  Rust's exactly rounded fixed-precision formatting, which is QuickJS's
  rounding, instead of big-integer arithmetic.
- `oxide-run --gc-stats` prints the collector's statistics (Table D).

## Phase 4 departures

### The host boundary

- `Host` holds the hooks a port must supply: `write`, `now_ms` and
  `local_tz_offset_minutes`. Compilation, module loading and the agents'
  script runner are function pointers on `Services` (`set_compiler`,
  `set_module_host`, `set_script_runner`) that receive the runtime's
  `Compiler`, rather than trait methods taking `&mut Runtime`: the host
  object lives inside the runtime it would have to borrow. The front end
  stays in the root crate either way.
- No promise-rejection hook: the CLI reports an uncaught rejection of a
  module's evaluation, and nothing else reads one. `import.meta` is an
  empty object; no program or admitted Test262 test reads a property of it.
- A test host reads a thrown value through `Runtime::thrown_summary`, which
  takes `name`, `message` and `stack` from own data properties (the
  prototype's `name`, then its `constructor`'s) without running
  JavaScript, as the current engine's Test262 worker does.

### Objects and the heap

- The builtin kinds share one `ObjectKind::Builtin` with an `InternalTag`
  and an `Internal` record (raw slots plus an `InternalData` payload: a
  table, a buffer, a view, a suspended frame) in one side arena, instead
  of an arena per kind. The wrappers, `Proxy`, `BoundFunction` and typed
  arrays keep their own `ObjectKind`, because the slow paths dispatch on
  it.
- A rope's halves live in `Heap.ropes`, keyed by the record, so a
  `StringRecord` stays at 40 bytes; a flattened or dead rope's entry is
  dropped at the next collection.
- Ephemerons mark in one pass: after the first drain, each weak map's
  entry with an unmarked key waits in a map keyed by the key, and marking
  the key pushes its values. The plan's rescan to a fixpoint took 25.7 s
  for a 10⁵-deep chain of `WeakMap` entries (Test262's
  `regress-1507322-deep-weakmap`); this takes 9 ms.
- The pristine-array test of `IterStart` reads `%Array.prototype%[@@iterator]`
  and `%ArrayIteratorPrototype%.next` and compares them with the
  intrinsics, instead of a validity cell: two own-slot lookups, no
  dependents list.

### Errors

- Errors get `stack` at construction, from the frame table and a sparse
  `DebugInfo.positions` table (a line and column from an instruction on)
  that the lowering derives from the front end's pc-to-source table. The
  plan's per-instruction `DebugInfo.lines` was never filled and is gone.
  The innermost frame's position is the instruction that called the
  running native (`VmStack.native_site`, set by the out-of-line native
  call); an error the loop itself throws has no position for that frame.
  A JSON module's syntax error carries its position in the module.
- `with` is an early `SyntaxError` raised by the lowering's surface check
  (the front end, shared with the current engine, still parses it), so a
  script or `eval` source that contains one fails before any of it runs.

### The loop

- New opcodes are appended at the end of `Opcode`: two inserted in the
  middle shifted the discriminants, changed how LLVM grouped `run_at`'s
  arms, and cost 1–6% of the instructions of every V8 v7 program.
- `ToPropertyKeyOf a, b, c` is the key conversion of a compound element
  assignment, which checks the base `c` before an object key converts; a
  primitive key passes through inline like `ToPropertyKey`'s.
- `ClearCell a` puts a captured lexical binding back in the TDZ when its
  scope is entered again (QuickJS's `set_loc_uninitialized`), keeping the
  cell that hoisted closures already hold; `CloseCell` still makes the
  fresh binding of a loop iteration.
- `GetSuperValue` and `IterStart` take their flag in `imm` (format
  `ABInt`).
- `CheckGlobalVar`, `DeclareGlobal` and `DefineGlobal` carry
  `opcode::global` flags in `b`: a function declaration's
  `CanDeclareGlobalFunction`, and `eval` code's configurable bindings.
