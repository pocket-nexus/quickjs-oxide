# oxide-core

The engine of quickjs-oxide: NaN-boxed values, typed arenas and a value
heap under a non-moving mark-sweep collector, shapes, register bytecode and
the loop that runs it, the builtins, and the `Runtime` that holds them. The
root crate compiles JavaScript for it (the front end and the register
lowering in `src/engine/compiler`) and embeds it (`src/engine/api`);
[workspace architecture](../docs/architecture.md) describes the crates and
how they fit together. This page is the core's design record: its module
layout, the rooting rule every heap user follows, and the design decisions
behind the bytecode, the loop, the caches and the heap.

The crate has `#![forbid(unsafe_code)]` and one dependency, the
regular-expression crate `oxide-regexp` (`regexp/`), which has none. The root
crate depends on it; it does not depend on the root crate.

## Layout

| Module | Content |
| --- | --- |
| `value` | `Value(u64)` (NaN-boxed, doubles favored), `Kind`, identifier newtypes |
| `heap::arena` | `Arena<T>`: `Vec<T>`, free list, side mark and live bitmaps |
| `heap::values` | the value heap: power-of-two ranges over one `Vec<Value>` |
| `heap::object` | `ObjectHeader`, `ObjectKind`, payload arenas |
| `heap::string`, `atom`, `symbol`, `bigint` | leaf records, interning, `PropertyKey` |
| `heap::gc` | root stack, handles, mark-sweep, trigger, stress mode, heap cap |
| `heap::buffer`, `table` | array-buffer storage and views; the ordered hash table of `Map`, `Set` and the weak collections |
| `heap::code` | the code units compiled at run time, owned and collected by the heap |
| `heap::report` | `MemoryReport`: what every arena, the value heap, the code and the caches hold, read without a collection |
| `shape` | transition tree, dictionary mode, validity cells |
| `bytecode` | `Insn` (8 bytes), `Opcode` and its operand formats, `CodeUnit`, `CodeStore` (the static units), `UnitRef` and `Units`, the builder, the publication validator, the disassembler |
| `vm` | the register file and frame table, the main loop (`interp`), conversions and operators (`ops`, `number`, `bigint`), property access (`property`), the inline caches (`ic`), regular-expression objects (`regexp`), the value printer (`print`) and the internal methods by kind (`object`, `proxy`, `typed`, `arguments`), iteration (`iter`), suspension (`suspend`, `async_generator`), jobs and promises (`jobs`), classes (`class`), modules (`module`), the compile host (`compile`), errors and the native ABI (`error`, `native`) |
| `builtins` | the realm's constructors, prototypes and globals as natives over the re-entrant `call`, one file per builtin object, plus `$262` (`test262`, `agent`) |
| `host` | the `Host` trait: output, clock, time zone, random seed, promise-rejection events and files |
| `embed` | the embedder's entries the root crate's API calls: objects, properties, conversions, value construction |
| `clock` | the monotonic clock the collector times its pauses with |
| `runtime` | `Runtime`: the heap, the static code store beside it, the single realm and the host services |

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
  passed straight into one call needs no rooting. Pinned atoms (those
  interned while the runtime is created, the keys of static units, and
  `Heap::intern_pinned`'s) are strong roots; every other atom follows the
  same rule as symbols, BigInts and plain strings. A compilation holds the
  atoms it interns until it ends.
- A `PropertyKey` converted from a string is a fresh atom when no atom has
  its content yet: held across JavaScript (a trap, a getter) or a second
  allocation, it is rooted (`object::root_keys`). `own_property_keys`
  returns its keys rooted.

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

## Values and the heap

- The root stack, the handle table and the fixed roots live inside `Heap`, so
  a safepoint in `Heap::alloc_*` sees them without extra parameters. `Runtime`
  holds the heap, the static code store, the realm and the host services;
  the dynamic units and the regular-expression programs are in the heap.
- The special values use payload bits 0–1 for the kind (Bool, Undefined, Null,
  Empty) and bit 2 for the boolean, so `kind()` is a branchless table lookup.
- Every value-heap range is a power of two, including large ones, so a header
  stores each range as a start and a `u8` exponent.
- `delete_property`, `set_prototype` and `set_property_flags` return
  `Result`: entering dictionary mode or replaying a shape allocates.
- Array indices from 2^31 to 2^32 − 2 are atoms whose record caches the index.
- Shapes count slots (`width`) apart from properties (`count`): an accessor
  is two slots. A reflag never changes a property between data and
  accessor; the literal paths delete and re-add instead.
- The builtin kinds share one `ObjectKind::Builtin` with an `InternalTag`
  and an `Internal` record (raw slots plus an `InternalData` payload: a
  table, a buffer, a view, a suspended frame) in one side arena. The
  wrappers, `Proxy`, `BoundFunction` and typed arrays have their own
  `ObjectKind`, because the slow paths dispatch on it.
- A rope's halves live in `Heap.ropes`, keyed by the record, so a
  `StringRecord` stays within 40 bytes; a flattened or dead rope's entry is
  dropped at the next collection.
- Ephemerons mark in one pass: after the first drain, each weak map's
  entry with an unmarked key waits in a map keyed by the key, and marking
  the key pushes its values.
- Errors carry an `ERROR` object flag set at creation, read by
  `Object.prototype.toString`.

## Register bytecode

- An `Insn` is 8 bytes: an `Opcode`, three byte operands `a`, `b`, `c` and a
  32-bit `imm`. Registers address a 256-register window; `LoadWide` and
  `StoreWide` reach the registers above it.
- Compare-jumps against a literal (`JLtI` and friends) carry an `i16` in
  `b | c << 8` and the absolute target in `imm`. The lowering materializes a
  literal outside the `i16` range. The arithmetic immediates (`AddI`, …)
  carry an `i32` in `imm`.
- The relational compare-jumps have negated forms (`JNlt`, `JNlte`, `JNgt`,
  `JNgte`): `if (!(a < b))` is not `a >= b` when an operand is NaN.
- A handler entry stores the register the thrown value lands in (`T(depth)`).
- `CheckInit` carries the binding's key for the `ReferenceError` message.
- `NewArray a, b, imm` takes its elements from the `imm` registers starting at
  `b`, where the stack code's `ArrayFrom` left them.
- `Swap` exchanges two registers, for cyclic stack shuffles. `DeclareGlobal`,
  `DeclareGlobalLet` and `DeclareGlobalConst` instantiate a script's global
  declarations; `CheckGlobalVar`, `DeclareGlobal` and `DefineGlobal` carry
  `opcode::global` flags in `b` (a function declaration's
  `CanDeclareGlobalFunction`, and `eval` code's configurable bindings).
- `ToPropertyKeyOf a, b, c` is the key conversion of a compound element
  assignment, which checks the base `c` before an object key converts; a
  primitive key passes through inline like `ToPropertyKey`'s.
- `GetSuperValue` and `IterStart` take their flag in `imm` (format
  `ABInt`).
- New opcodes are appended at the end of `Opcode`: an opcode inserted in the
  middle shifts the discriminants, changes how LLVM groups `run_at`'s arms,
  and costs 1–6% of the instructions of every benchmark program.
- There is one `FunctionBytecode` record per published code unit, shared by
  every closure over it. It holds the constant table and the child records'
  indices (as `Int32` values in a second value-heap range), so a live function
  keeps alive every function it can create. A tree is created under one
  safepoint by `Heap::alloc_bytecode_function` (via `Runtime::instantiate`),
  because a record is not a value the root stack can hold; `MakeClosure` uses
  `Heap::alloc_closure`, whose record is reachable through the running
  function.

## Bindings and cells

- Bindings captured by an inner function live in *cells* (one-slot scope
  objects). The lowering consumes the front end's finished stack code, where
  block structure is expanded into QuickJS's `close_loc` discipline; cells
  reproduce that discipline exactly. The binding's register holds its cell
  (`NewCell`, `LoadCell`, `StoreCell`), the stack code's `CloseLocal` is
  `CloseCell` (a fresh cell holding the current value: a `for (let …)`
  iteration), and a closure holds one cell per captured binding, listed by
  the child unit's `captures` and read with `LoadCaptured`/`StoreCaptured`.
  A function reaches its captures through its function object; a frame has
  no scope field.
- `ClearCell a` puts a captured lexical binding back in the TDZ when its
  scope is entered again (QuickJS's `set_loc_uninitialized`), keeping the
  cell that hoisted closures already hold.
- A function whose locals do not fit the 256-register window keeps the rest
  in the slots of a spill scope (`NewSpill`, `LoadSlot`, `StoreSlot`, and
  `Capture::Slot` for a child capturing one), held in the register after the
  window's locals, with the temporaries above it: a call's window starts at a
  temporary, so a local placed past the temporaries would be overwritten. A
  function whose operand stack does not fit either keeps its deeper
  temporaries in slots of the same scope, reached through eight scratch
  registers above the ones that fit; a call that does not fit in the scratch
  registers is `CallSpread` over an array of its arguments.
- A mapped `arguments` object keeps its parameters' cells in a scope object
  named by its `payload` (slot 0 the mapped count, then one cell per mapped
  parameter, `Value::EMPTY` once unmapped). An unmapped one's payload is
  `NONE`. Captured parameters are cells, so `NewArguments` and `Rest` read a
  parameter register through its cell when it holds one.

## Calls and frames

- Calls follow the stack code's operand order: `Call a, b, c` has the callee
  in `b` and the arguments from `b + 1`, so the callee's window starts at `b`
  (its `this`, written by the call); `CallMethod` and `New` have receiver or
  callee and new.target in `b`, `b + 1`, and the window starts at `b + 1`. No
  argument moves either way.
- A `Frame` is 32 bytes: it keeps the running function's bytecode record next
  to its unit, the caller's destination as an absolute register index, and
  `argc`.
- A function that reads its actual arguments (the `ARGUMENTS` unit flag, for
  `arguments` and rest parameters) keeps the ones past its parameters above
  its registers when it is entered.
- Entering a frame clears the window past the arguments to `undefined`:
  locals need it anyway, and a register above a returned frame's `top` may
  name a record freed since, which must not be marked once the window covers
  it again. Temporaries below `top` are marked conservatively. The dead
  registers above a returned frame are cleared when a frame resumes over them
  if a collection has run since they were last above `top`
  (`VmStack::stale_from`).
- The register file holds at most `DEFAULT_STACK_LIMIT` (2^20) values by
  default (`RuntimeOptions::stack_limit`); a call past it throws
  `InternalError: stack overflow`, QuickJS's error.
- A conversion or a native may call back into JavaScript through a nested
  loop on the frames above its own; such re-entries share a native-stack
  budget (1 MiB by default) measured from stack addresses, as QuickJS's
  `js_check_stack_overflow` does.
- `TailCall` reuses no frame: the callee's frame is marked so its result is
  also its caller's.
- A thrown value travels as an unrooted `Throw`; the unwinder stores it in
  the handler's register (or returns it to the embedder) before anything
  allocates.
- The opcodes no hot path runs (accessors in literals, the spill scope,
  `arguments`, rest, `new.target`, regular-expression literals, `for-in`
  setup, spread elements and the like) share one loop arm that calls an
  out-of-line `rare_op`, which keeps `vm::run_at` under its size gate.

## Inline caches

- The `IcStore` lives inside the `Heap`, beside the register file:
  collection happens inside heap safepoints, and the store is pruned after
  every marking before an index is recycled. The loop reads it as
  `cx.heap.ics`; the static code store stays outside the heap.
- Cache sites are numbered by `CodeBuilder::finish`: the named-property
  sites first in instruction order (0 to 254 in `c`, the rest sharing the
  reserved site 255), then the global forms (`b | c << 8`), then `New`
  (`imm`). `Compiler::publish` reserves the entries, with each site's kind
  for pruning, and assigns `ic_base`: the first free range that fits, since
  a freed dynamic unit returns its range.
- The `GetField` hit reads the object, cell and value arenas as fields of
  the heap, disjoint from the register file, so the loop keeps its register
  window across a hit.
- A string-receiver entry is guarded by the cell of `%String.prototype%`'s
  own chain (`Heap::chain_cell`), which any change to it or to
  `%Object.prototype%` clears.
- The global generation lives in the `Heap` (`global_generation`): every
  insert, remove and reflag of the global object's dictionary bumps it, so a
  global hit is one load and one compare against the entry. On wrapping to
  the empty entry's tag the generation restarts at 0 and every global entry
  is cleared.
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
- `GetField` has a `length` entry (`LENGTH_TAG`) that reads an array's
  stored length or a string's unit count.
- A prototype hit on a dictionary-mode holder is cached: any change to a
  prototype clears the cell, dictionary or not. A dictionary-mode receiver
  is not cached.
- A megamorphic `GetField` site probes a shared table of 4,096 entries,
  keyed by receiver shape and property key, and fills it from its misses.
- The `ic-stats` counters exist with the feature and in debug builds, so
  tests see hits. `Runtime::ic_report` prints them, and `qjs --ic-stats`
  (CLI feature `ic-stats`) prints the report to stderr at exit.

## The language surface

- `Function.prototype.call` and `apply` are natives that the `Call` arm
  rewrites (an out-of-line `rewrite_call`) into a call of their receiver;
  `vm::via` records the frames QuickJS would show for them in a backtrace.
- `ForInNext` jumps to the loop body with a key and falls through when the
  enumeration is exhausted, the shape of the stack code's `ForInNext` and
  its `IfFalse`. The enumeration is a `ForInIterator` object: the key
  snapshot in its elements (index keys as `Int32`, turned into strings as
  they are yielded), the object and the count in two raw slots, the cursor
  in its payload.
- Regular-expression literals and `new RegExp` share one store keyed by the
  `(source, flags)` atoms, inside the heap: a `RegExp` object's payload names
  its program, and a collection frees the programs no live object names
  (with their keys). A program compiles when a literal
  is first evaluated or `RegExp` is called, and runs on a string's own
  one-byte or two-byte storage. `split` with a regular expression searches
  forward from each position, which finds the specification's matches and
  leaves the user's `lastIndex` alone.
- The array-iteration fast path of `IterStart` reads
  `%Array.prototype%[@@iterator]` and `%ArrayIteratorPrototype%.next` and
  compares them with the intrinsics: two own-slot lookups, no dependents
  list.
- `with` is an early `SyntaxError` raised by the lowering's surface check
  (the front end parses it), so a script or `eval` source that contains one
  fails before any of it runs.

## The realm and the builtins

- Natives are one `Native` enum generated with its table of entry points and
  constructor flags (`builtins::natives!`), indexed from the function record;
  `vm::native` holds `CallSite` (receiver, arguments, and whether `new`
  called) and the dispatch, which cuts the root stack back after every
  native so natives root intermediate values on it freely.
- `Number.prototype.toString(radix)`, `parseInt` and `parseFloat` use exact
  algorithms on the core's own `Big` (`vm::radix`); radix-10 formatting of
  non-integers uses them too, since Rust's shortest formatting differs from
  QuickJS on ties and on some powers of two.
- Constructed receivers and empty object literals reserve at least four
  slots; a constructor site learns sizes only past that.
- `Mul` on doubles keeps a double result, as `Add` and `Sub` do. `Div`
  gives an `Int32` when the quotient is one.
- `qjs --gc-stats` prints the collector's statistics.

## The host boundary

- `Host` holds the hooks a port supplies: `write`, `now_ms`,
  `local_tz_offset_minutes`, `random_seed`, `promise_rejection` and
  `read_file`. Compilation, module loading and the agents' script runner are
  function pointers on `Services` (`set_compiler`, `set_module_host`,
  `set_script_runner`) that receive the runtime's `Compiler`: the host
  object lives inside the runtime it would have to borrow. The front end is
  the root crate's.
- A test host reads a thrown value through `Runtime::thrown_summary`, which
  takes `name`, `message` and `stack` from own data properties (the
  prototype's `name`, then its `constructor`'s) without running
  JavaScript.
- Errors get `stack` at construction, from the frame table and the unit's
  sparse `positions` table (a line and column from an instruction on), which
  the lowering derives from the front end's pc-to-source table. A JSON
  module's syntax error carries its position in the module.
