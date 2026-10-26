# Execution

This page describes how the core (`quickjs-oxide-core`, in [`core/`](../../core/README.md)) runs register bytecode: the loop, its frames and register windows, the inline caches, iteration, suspension and the native ABI. The main files are `core/src/vm/interp.rs` (the loop), `vm/ic.rs` (the caches), `vm/iter.rs`, `vm/suspend.rs`, `vm/native.rs` and `core/src/bytecode/`. The heap, shapes and the collector are in [Heap](../heap.md); the core's design record is [`core/README.md`](../../core/README.md).

## Code

The root crate's front end parses and resolves source into the HIR; the middle end builds each function's SSA graph and its lowering (`src/engine/mir/lower`) turns it into a `CodeUnit` of the core. An instruction (`Insn`) is 8 bytes: a typed `Opcode`, three 8-bit operands `a`, `b`, `c` and a 32-bit `imm` (a constant index, an absolute jump target, a property key, a scope slot or an `i32` literal, as the opcode's `Format` says). A unit also holds its exception handlers (`start..end`, the target and the register the thrown value lands in, innermost first), its register, parameter and local counts, its flags (`STRICT`, `CONSTRUCTOR`, `ARGUMENTS`, `GENERATOR`, `ASYNC`, …) and the number of its cache sites.

`Runtime::publish` runs the validator (`bytecode/validate.rs`) once per unit and appends it to the `CodeStore`: every register operand is below `register_count`, every call window and `NewArray` range fits, every target, constant, child and capture index is in range. The loop then indexes without further checks of its own; a validator failure is an internal error of the lowering, never a JavaScript exception. `qjs --disassemble FILE` prints the register code.

New opcodes are appended at the end of `Opcode`. Inserting one in the middle shifts the discriminants and changes how LLVM lays out the loop's arms, which moves the instructions of every V8 v7 program by 1–6%.

## Frames and register windows

The register file (`VmStack::registers`) is one `Vec<Value>`; the frame table (`VmStack::frames`) holds one 32-byte `Frame` per activation:

| Field | Content |
| --- | --- |
| `unit`, `bytecode`, `function` | the running code unit, its `FunctionBytecode` record (constants, children) and the function object (its captured cells) |
| `base` | the absolute index of the frame's register 0 |
| `return_pc`, `return_dst` | where the caller resumes and the absolute register that receives the result |
| `argc` | the actual argument count |
| `flags` | `ENTRY` (entered from Rust: its return leaves the loop), `CONSTRUCT`, `TAIL`, and the `VIA_*` backtrace marks of a rewritten `call` or `apply` |

A frame's window is register 0 (`this`), the parameters, a hidden `new.target` local when the function reads it, the locals, then the temporaries. A function with the `ARGUMENTS` flag (`arguments` or a rest parameter) keeps the arguments past its parameters above its registers, where `NewArguments` and `Rest` read them. A call's operands occupy consecutive registers, where the lowering computes them: `Call a, b, c` has the callee in `b` and `c` arguments from `b + 1`, and the callee's window starts at `b`, where the call writes `this`; `CallMethod` and `New` have the receiver or `new.target` in `b` and start the window at `b + 1`. No argument is copied either way.

Entering a frame (`enter_frame`) reserves the window, clears every register past the arguments to `undefined`, coerces a sloppy function's `undefined` or `null` receiver to the global object (a primitive one to its wrapper), pushes the frame and sets `top` to the window's end. `registers[..top]` are roots. Registers above a returned frame are dead; when a frame resumes over registers a collection may have freed since they were last below `top` (`stale_from`), `raise_top` clears them before they become roots again.

`VmStack::reserve` keeps `SPAN` (512) values of padding past every frame's end, so the loop takes the running window as a fixed-size `&mut [Value; 512]` and an 8-bit operand (or a call's `b + 1`) needs no bounds check. The register file grows by doubling up to `DEFAULT_STACK_LIMIT` (2^20 values); a call past it throws `InternalError: stack overflow`.

### Past the 256-register window

Register operands address 256 registers (`bytecode::WINDOW`). The lowering fits a function that needs more in `fit` (`src/engine/mir/lower/mod.rs`):

- **Variables past the window** live in the slots of a spill scope (`NewSpill`, `LoadSlot`, `StoreSlot`, and `Capture::Slot` for a child that captures one), held in the register after the window's locals, with the temporaries above it. Locals are never placed above the temporaries, where every call's window would overwrite them. `fit` spills local variables from the last one until the values fit beside the others; a `GetVar` of a spilled variable is a `LoadSlot` and a `SetVar` a `StoreSlot`.
- **Values past the window.** When the values need more registers than the window holds even with every local variable in the spill scope, `lower/spill.rs` turns the longest-lived instruction results into variables of their own, written once after their definition and read right before each use, and the layout puts them in the spill scope too. It moves as many as the registers overflow, doubling the count until the function fits.

A function inside the window lowers exactly as it would without this path. The `LoadWide` and `StoreWide` opcodes (registers above the window by absolute index) exist in the core and its tests; the lowering emits neither.

## The loop

`vm::interp::run_at` runs the top frame from an instruction until the entry frame returns. It is one `'dispatch: loop` over `match insn.op` with the frame-derived values (`code`, `base`, `strict`, `ic_base`) in locals and the window in `regs`.

- **Inline arms** cover moves and constants, `Int32` and double arithmetic and comparisons (an overflow or a non-number takes `ops`), the compare-jumps against registers and against `i16` literals, `GetField`, `SetField` and the global forms' cache hits, dense and typed-array element reads and writes, cells and captures, `NewObject`, calls and returns.
- **The window and the heap.** `regs` borrows the register file inside the heap, so an arm that calls into the heap copies its operands out first and takes the window again afterwards (the `set!` and `write!` macros, and the common tail). The cache hits take the heap's arenas as separate fields, disjoint from the register file, and keep the window.
- **Slow paths** are `#[inline(never)]` functions in `ops`, `property`, `object` and the other modules, returning `Result<_, Throw>`. Their result falls into the common tail: success retakes the window and dispatches, a throw unwinds.
- **The rare-op split.** Opcodes no hot path runs share out-of-line arms: `rare_op` (accessor and method definitions, the spill scope, `arguments`, rest, `new.target`, regular-expression literals, `for-in` setup, spread elements, the error throws, `import()`, `ConcatN`), `iter::iter_op` (iteration), `class::class_op` (classes, private names, super and reference forms) and `suspend_op` (generators and `await`). A new opcode enters `run_at` inline only when a profile of a suite program shows it hot.
- **Unwinding.** A thrown value travels as an unrooted `Throw`. The tail searches the running unit's handlers for the faulting instruction, stores the value in the handler's register and continues at its target; otherwise it pops the frame and repeats in the caller, until a frame marked `ENTRY` returns the throw to Rust. `error::place_frame` records the faulting position for the backtrace on every throw.

### Calls and returns

A call or `new` of a bytecode function stays in the loop: the arm builds the callee's `Frame`, enters it and continues at the callee's instruction 0, with no Rust recursion. For `new`, `construct_this` allocates the receiver through the constructor's cache (below) unless the constructor is derived. `Function.prototype.call` and `apply` are natives the call arm rewrites (`rewrite_call`) into a call of their receiver, moving the window onto `thisArg` or spreading the argument list. `CallSpread` and `NewSpread` expand their array into the registers first. Natives and exotic callables (bound functions and proxies, which run through the re-entrant entry points below) go through the out-of-line `call_other`.

`Return` from a frame with none of `ENTRY`, `CONSTRUCT` or `TAIL` set pops it, writes the value into the caller's `return_dst` and resumes the caller in place. The other frames go through `finish_return`, which applies the constructor's receiver rule (a non-object result yields the receiver), teaches the `New` site its receiver size, returns through the frames a tail call left, or leaves the loop at an entry frame. `TailCall` does not reuse the frame; the compiler emits no tail call.

Rust re-enters JavaScript through `vm::call` and `vm::construct`: the receiver and arguments are written above `top` (rooted from that point), `call_from_rust` pushes an `ENTRY` frame and runs a nested `run_at` on the frames above its own. Each re-entry counts against a native-stack budget (`NATIVE_STACK_BUDGET`, 1 MiB measured from stack addresses, or a depth limit the embedder sets for a host whose stack cannot be measured); past it the call throws `InternalError: stack overflow`.

### The size gate

`run_at` stays under 16,384 bytes, read with `nm --size-sort` on the release `qjs` (symbol `oxide_core::vm::interp::run_at`). It is 15,226 bytes. A change that crosses the gate moves code out of line before it merges; see [Execution principles](principles.md).

## Inline caches

Each cache site has one 16-byte `IcEntry` (`w0`–`w3`) in the `IcStore`. The store lives in the `Heap`, beside the register file, rather than in a `CodeUnit`: the loop holds the unit's code borrowed across every heap call, and the collector prunes every entry that names a dead shape, object or cell after each marking, before an index is recycled. Entries are weak.

`CodeBuilder::finish` numbers a unit's sites: the named-property sites first in instruction order (0 to 254 in `c`; the rest share the reserved site 255, which is never filled), then the global forms (`b | c << 8`), then `New` (`imm`). `Runtime::publish` reserves the unit's entries from its `ic_base`.

| Kind | Opcodes | Entry | Hit |
| --- | --- | --- | --- |
| Get | `GetField` | receiver shape, holder (`NONE` for an own property), slot, validity cell | own: one shape compare and a slot load; prototype or string receiver: also the cell's `valid` flag, then a load from the holder |
| Set | `SetField` | shape before, shape after, slot, validity cell | same shape: a slot store; transition: the shape matches, the slot is within the object's reserved range, the object is extensible and not used as a prototype, and the chain's cell holds; then the new shape and the store |
| Global | `GetGlobal`, `GetGlobalOrUndefined`, `SetGlobal` | global generation, slot, property flags, holder | one compare of the heap's `global_generation` and a load (`SetGlobal` also checks `WRITABLE`) |
| Construct | `New` | constructor shape, `prototype` slot, receiver slots | the constructor's shape matches: its `prototype` slot read live, the receiver allocated with the learned slot count |

Details of the hit paths:

- **Tagged `GetField` entries.** `LENGTH_TAG` reads an array's stored length or a string's unit count with no shape to compare; `STRING_RECEIVER_TAG` caches a property found on `%String.prototype%` or above, guarded by that prototype chain's cell.
- **Polymorphic and megamorphic sites.** A `GetField` or `SetField` site is monomorphic until a second shape arrives, then links a row of four entries (`POLY`) that the out-of-line path (`property::get_cached`, `property::set_cached`) probes in order, then turns megamorphic. A megamorphic `GetField` site probes a shared direct-mapped table of 4,096 entries keyed by receiver shape and key, and fills it from its misses.
- **What is never cached.** Accessors are not cached at `GetField`; a dictionary-mode receiver is never cached, while a prototype hit on a dictionary-mode holder is (any change to a prototype clears its cell). A transition is cached only when no prototype has the key as an accessor or read-only property.
- **Globals.** Every insert, remove or reflag on the global object's dictionary bumps `global_generation`; a value write does not. A wrap of the generation clears every global entry. A miss looks up the global lexical environment first, then the global object's own data property.
- **Construction.** A constructed receiver or empty object literal reserves at least four slots. When a constructor's frame returns to its `New`, `learn_receiver_size` raises the entry's slot count to the receiver shape's width (capped at twice the dictionary threshold), so the next construction reserves what the constructor added.

Element access has no cache: `GetElem` and `SetElem` read and write an array's or ordinary object's dense elements inline for a non-negative `Int32` key, and a typed array's through `typed::fast_get` and `fast_set`. A hole is filled inline only while no object used as a prototype has an index property or is exotic (`Heap::holes_writable`).

With the `ic-stats` feature (and in debug builds) the store counts hits, polymorphic hits, fills and unfilled misses per kind, misses per site and iteration starts; `qjs --ic-stats` prints `Runtime::ic_report` at exit. Release builds without the feature carry no counter.

## Iteration

An iterator record is three consecutive registers. `IterStart a, b` fills them for the iterable in `b`:

- **Pristine array**: an `Array` whose prototype is `%Array.prototype%`, with no own `@@iterator`, while `%Array.prototype%[@@iterator]` is the intrinsic `values` and `%ArrayIteratorPrototype%.next` the intrinsic `next` (`iter::is_pristine_array`: two own-slot lookups, no validity cell). The record is the array, the mark `Value::EMPTY` and the index. No iterator object and no `{value, done}` result is allocated; the length is read live and a hole is read through the property path, so the walk is unobservable.
- **Protocol**: the iterator, its `next` method and a done flag.

`IterNext a, b, c` writes the next value of record `b` to `a` and whether it is done to `c`; `IterClose` and `IterCloseThrow` do nothing for a pristine or finished record. `for-of` takes the pristine form; the front end's other iterator starts set `imm` and always run the protocol. The natives use the same mechanism through `iter::get_iterator`, `step` and `close` (an `Iter::Array` or `Iter::Protocol`): spread, `Array.from`, the typed-array constructors, `Promise` combinators, the collection constructors, `Object.fromEntries` and others. The `IterStart` counters of `ic-stats` report how many starts took each form.

## Suspension

Generators, async functions and async generators copy their frame out of the register file when they suspend and back in when they resume (`vm/suspend.rs`). The activation record is the generator or async-state object itself: internal slot 0 the function, slot 1 the kind's own state (an async function's promise, an async generator's request queue), the saved registers after them, and a `SavedFrame` header (unit, bytecode record, resume `pc`, `argc`, state `START`, `YIELD`, `EXECUTING`, `COMPLETED` or `AWAIT`).

- `InitialYield` creates a generator's record, saves the frame and returns the generator to the caller. `AsyncStart` creates an async function's promise and record, saves the frame and runs the body at once through `resume`.
- `Yield`, `YieldStar` and `Await` save the window `base..frame_end` into the record found on `VmStack::active` and leave the nested loop. `Await` first performs `PromiseResolve` and a reaction whose job resumes the record, so a throw there is catchable at the `await`.
- `suspend::resume` copies the registers back at the current `top`, pushes an `ENTRY` frame from the header, writes the delivered value (and at a `yield`, the resume kind: next, return or throw) into the registers the lowering reserved, and runs `run_at` from the saved `pc`; a throw delivery runs `run_throwing`, which starts at the resume point's handler.

The cost is a copy of the live window per suspension and per resume. A suspended frame is ordinary heap data, so the collector traces it and the stress mode covers it.

## The native ABI

Builtins are natives: one `Native` enum generated with its table of entry points and constructor flags (`builtins::natives!`), indexed from the function record. A native is `fn(&mut Cx, CallSite) -> Result<Value, Throw>`, where `Cx` holds the heap mutably and the code store and realm shared.

`CallSite` names where the native finds its inputs in the register file, by index: `this`, `args` and `argc`, whether `new` called it with which `new.target`, and the function object called (whose data object holds a native closure's slots, read with `data` and `set_data`). `arg(i)` reads an argument or `undefined`; `args` borrows them as a slice. The loop's `call_native` writes `this` into the window's first register and raises `top` over the arguments, which may reach past the caller's registers after an `apply`, so they stay rooted for the call.

`native::call` marks the root stack before the native and truncates it after, so a native roots what it holds across an allocation or a call (`heap.roots().push`) without releasing it. A native calls back into JavaScript as straight-line Rust through `vm::call` and `vm::construct`. While a native runs, `VmStack::natives` records the calling frame and instruction for the backtrace; the natives QuickJS runs without a frame of their own (`Native::frameless`: promise resolving functions and data closures) are not listed.

## Verification

- The loop, frames, calls and suspension: `core/tests/vm.rs` and the corpus in `core/tests/corpus`, also under `cargo test -p quickjs-oxide-core --features gc-stress` and `scripts/checks/check-gc-stress.py`.
- Caches: `core/tests/ic.rs` and `src/engine/mir/ic_tests.rs`.
- Instruction encoding and validation: `core/tests/bytecode.rs`.
- The lowering and the register window: `src/engine/mir/run_tests.rs` and `window_tests.rs`, also under `cargo test -p quickjs-oxide --features gc-stress --lib mir::`.
- Language semantics: `cargo test --locked --workspace --all-targets` and the [Test262 guide](../test262.md).
- Cost: [Measurement method](measurement.md) and the gates in [Execution principles](principles.md).

