# New core, phase 3: property system, inline caches and the V8 v7 slice

This page is the implementation plan for phase 3 of the new core tracked in issue #122 ("four cache kinds on `ShapeId`; prototype validity guards; constructor-sized property ranges; dense arrays; typed-array fast paths", exit: "runs all eight V8 v7 benchmarks; same-machine comparison against QuickJS", with the go/no-go gate of the whole plan). Like the [phase 1](new-core-phase1.md) and [phase 2](new-core-phase2.md) plans it is written for whoever implements the phase: it fixes the decisions that are made, lists the ones that are deliberately open with the milestone that decides each, and gives every milestone an acceptance check that can be run without judgment calls. Day estimates order the work; they are not commitments.

Read first: [#122](https://github.com/pocket-nexus/quickjs-oxide/issues/122) for the design and the phase order, [phase 0](new-core-phase0.md) for the comparison base and the dropped language features, the [baseline](baseline.md) for the numbers the gate is measured against, [phase 2](new-core-phase2.md) and its [results](new-core-phase2-results.md) for the loop and the lowering this phase extends, [`core/README.md`](../../core/README.md) for the rooting rule and the departures from the earlier plans, and [Measurement method](measurement.md).

## 0. Scope

**In scope.** Inline caches for named property reads, named property writes, global reads and writes, and construction, as a side store next to the code store; accessor properties in the heap and the slow paths; the language surface the eight V8 v7 programs need beyond phase 2 (`arguments`, rest parameters, spread of arrays, `Function.prototype.apply` and `call` inside the loop, `for-in`, regular-expression literals); the builtins those programs call, as leaf natives; the regular-expression engine as a crate both cores use; the measurement of section 9 and the go/no-go decision of #122.

**Out of scope.** Typed arrays and their fast paths: no V8 v7 program uses them, they need the `ArrayBuffer` port, and they move to phase 4 with it (section 10). Natives that call back into JavaScript through the Step/Resume machines (`Array.prototype.sort` with a comparator, `replace` with a function, `forEach` and friends): no program in the slice needs one, and the ABI is phase 4's. Ropes (phase 4, with a pull-forward trigger in section 11). Classes, generators, async, destructuring, iterators, modules, `eval`, `new Function`, wrapper objects (`new String(…)`), the global lexical environment, the embedder API: phase 4. Nothing in the current engine changes except the move of the regular-expression engine into its own crate (section 6.1).

**What phase 2 learned that shapes this phase.** The [exit tables](new-core-phase2-results.md#m6-exit-tables) put the new core at 1.12× QuickJS's time and 1.32× its instructions over the fixed probes, with the uncached paths as the largest remaining items: `prop_read` 1.47×, `call0` 1.78× (an uncached global lookup per call), the depth-3 prototype read 2.8×, the constructor case 9% above QuickJS with an uncached `prototype` lookup and two uncached `this.x =` stores per iteration. The [census](new-core-phase2-results.md#the-benchmark-programs) of the suite programs says what blocks the eight V8 v7 programs today: every one is declined for the harness's `print` shim (a rest parameter and a spread call), the five that would otherwise lower stop at their first builtin (`Math`), RayTrace and Earley-Boyer need `arguments`, RegExp needs regular-expression literals. One function in the suite exceeds the 256-register window and it is not hot, so the window layout stays.

A second census, of the built programs themselves (`target/m6/v8/*.js` of the measuring tree, the baseline's `suite_matrix.py build`), decides what is hot rather than merely present:

| Program | Hot paths beyond phase 2 | Setup-only |
| --- | --- | --- |
| Richards, DeltaBlue | prototype-method calls; DeltaBlue chains constructors with `Super.call(this, …)` and `Super.prototype.m.call(this)`, and defines `Object.prototype.inheritsFrom` | `Math.random` is replaced by the harness |
| Crypto | `Array` element loops, `new Array()`, `charAt`, `charCodeAt`, `String.fromCharCode`, `Number.prototype.toString(16)`, `parseInt(s, 16)`, `substr`, `substring`, `Math.floor`, `Math.min`, `Math.pow` | |
| RayTrace | every object is constructed through `this.initialize.apply(this, arguments)` (a sloppy, mapped `arguments` object handed to `apply`), `Math.floor`, `Math.sqrt`, `Math.abs` | `Object.extend` with `for-in` |
| Earley-Boyer | `arguments` (102 uses), `apply` (13), `instanceof` (70), `delete` on hash tables, string `slice`, `substring`, `charAt`, `charCodeAt`, `toLowerCase`, `indexOf`, `concat` (also `"".concat.apply("", a)`), `String.fromCharCode`, `Math.round`, `parseInt` | `new RegExp`, `exec`, `replace`, `split`, `for-in`, a never-taken `eval` call |
| RegExp | 366 regular-expression literals, `exec`, `replace` with string replacements, `split` and `match` with regular expressions, `String.fromCharCode`, `charCodeAt`, `substring` | |
| Splay | allocation-heavy tree of objects, `String(key)`, `Math.random` (the harness's), `throw new Error` | |
| NavierStokes | `new Array(size)` filled with doubles, `Math.sqrt` | |

Every program also compiles, but never runs, the harness's `Date`, `setTimeout`, `toFixed` and `toPrecision` paths; they need no builtin. No program uses typed arrays, getters or setters, `sort`, `forEach`, `map`, or a `replace` with a function. The only natives that must run JavaScript are `apply` and `call`, and section 5.2 keeps them inside the loop.

## 1. Location

Everything new lives in `oxide-core` and the lowering, with one exception: the regular-expression engine moves out of the root crate into a workspace crate both cores use (section 6.1).

| Module | Crate | Content |
| --- | --- | --- |
| `vm::ic` | `oxide-core` | `IcStore`, the entry formats, fill and hit paths, invalidation at sweep, the `ic-stats` counters |
| `vm::property` | `oxide-core` | accessor-aware get and set, the `for-in` enumeration, `arguments` element access |
| `builtins` (new) | `oxide-core` | the realm's constructors and prototypes, one file per builtin object, leaf natives only |
| `heap::object` | `oxide-core` | the new object kinds and their payload arenas: `Arguments`, `RegExp`, `ForInIterator` |
| `compiler::register` | root crate | site numbering, the new opcodes, mapped-`arguments` parameters as cells |
| `regexp/` (new) | `oxide-regexp` | the compiler and executor of `src/regexp` and the Unicode tables of `src/source/unicode`, over `&[u16]` |

Gates that change, in the pull request that moves the engine: `scripts/checks/test262-host-metadata.py` admits `oxide-regexp` as a package, as a dependency of `quickjs-oxide` and as the single dependency of `oxide-core`; `scripts/checks/check-source-layout.py` requires the `src` directories to be exactly `engine`, `source` and `regexp` today and drops `regexp` from that set (it does not check `core/`, so `core/src/builtins` needs no admission); `dev-support/test262/current.conf` adds `regexp/` to the engine fingerprint trees as [commit 07140392](../../dev-support/test262/README.md) added `core/`. Candidate A of the host-boundary check still holds: providers depend on the engine, never the reverse, and neither `oxide-core` nor `oxide-regexp` depends on the root crate.

Delivery: one pull request per milestone, stacked on #140 in milestone order, each with its acceptance check in the description, gated by `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `cargo test -p oxide-core --features gc-stress`, the two-core corpus under `new-core-gc-stress`, `check-new-core-stress.py`, and, for the pull requests that touch the root crate outside `compiler/register` (the regular-expression move), the Test262 focused and full runs of the current engine. **Before M1 stacks on #140**, #140's `fast` job must pass: `test262-host-metadata.py` fails there because `apps/cli/tests/oxide_run.rs` (added by M5) is auto-discovered as a fourth integration target of the CLI crate and the check's expected set is `{cli, oracle, profiling}`; the fix is to add `oxide_run` to that set in the M5 pull request.

## 2. Inline caches

### 2.1 Where the caches live

The loop holds `code: &[Insn]` borrowed from the `CodeStore` across every heap call, so a cache cannot be a field of a `CodeUnit` behind that shared borrow. Caches live in a third store next to the heap and the code store:

```rust
pub struct Runtime { heap: Heap, code: CodeStore, ics: IcStore, realm: Realm, host: Host }
pub struct Cx<'a> { heap: &'a mut Heap, code: &'a CodeStore, ics: &'a mut IcStore, realm: &'a Realm, host: &'a mut Host }

#[repr(C)] #[derive(Clone, Copy)]
pub struct IcEntry { pub w0: u32, pub w1: u32, pub w2: u32, pub w3: u32 }   // 16 bytes, read per opcode

pub struct IcStore { entries: Vec<IcEntry>, poly: Vec<[IcEntry; POLY]>, stats: Option<Box<IcStats>> }
```

- A code unit records `ic_base: u32` and `ic_count: u32`; `CodeStore::publish` reserves `ic_count` entries in the `IcStore` (append-only, like the store itself) and the validator checks every site index against `ic_count`. Sites are numbered per unit by the lowering, in instruction order.
- **Site operands.** No encoding changes: `GetField a, b, imm = key` and `SetField a, b, imm = key` carry the site in `c` (0–254; 255 means "no cache", for units with more than 255 named-property sites, where the lowering caches the first 255 in instruction order); `GetGlobal` and `SetGlobal` carry it in `b | c << 8`; `New` carries it in `imm`, which the `Call` format leaves free. `GetElem` and `SetElem` have no free operand and need no cache: their dense-element admission is already a kind check and a bounds check.
- **Cleared at sweep.** Shape, cell and object indices are recycled, so an entry that names a dead record would hit on a recycled id. After marking, the collector clears every entry whose shape, holder, cell or constructor is unmarked (`IcStore::prune(&MarkBits)`, one linear pass over the store). Entries are therefore weak: a cache never keeps a shape or a prototype alive. Under `gc-stress` the post-collection verifier additionally checks that every filled entry names live records.
- **Polymorphism.** An entry is monomorphic inline. On a miss with a different shape the site moves to the `poly` table (`w0 = POLY_TAG`, `w1` the index) with up to four entries, probed in order on the out-of-line path; the fifth distinct shape marks the site megamorphic and it stops recording. The inline hit path tests the first entry only, so a monomorphic site costs one compare more than phase 1's measured 12-instruction hit. Whether a megamorphic site ever re-records is decided by M5's `ic-stats` (section 2.6).

### 2.2 Named reads: `GetField`

Entry: `shape` (the receiver's `ShapeId`), `holder` (`NONE` for an own hit, else the `ObjectId` that holds the property), `slot`, `cell` (the receiver's prototype-chain `ValidityCellId`, phase 1 §5; `STATIC_CELL` for own hits).

Hit path, in the arm: receiver is an object, `shape_of(receiver) == shape`, then either `holder == NONE` → `get_slot(receiver, slot)`, or `cell_is_valid(cell)` → `get_slot(holder, slot)`. Nothing else is tested: a shape change on the receiver changes `shape`, and any add, delete, reflag or `[[Prototype]]` change on any object of the chain clears `cell` (phase 1's dependents list). A value stored into the holder's slot is read live.

Fill, on the miss path (`property::get_cached`, `#[inline(never)]`): the uncached walk of phase 2 runs; if it ends in a data property on an object whose shape is not a dictionary shape, the entry is written, with `cell = heap.validity_cell(receiver)` for a prototype hit. Accessor results, dictionary holders (the global object, deleted-from objects) and complete misses are not cached in phase 3: the slow path returns the value and the site stays as it was. A complete-miss cache (the `undefined` result with a cell guard) is an M5 option if the profile shows one (`typeof x.y === "undefined"` probing is common in RayTrace's `Class.create` idiom).

**Primitive receivers.** `s.charCodeAt(i)` and `s.length` are the hottest named reads in Crypto, Earley-Boyer and RegExp. A string receiver's method lookup goes to `%String.prototype%`; the entry stores `shape = STRING_RECEIVER_TAG`, `holder = %String.prototype%`, `slot`, and `cell = validity_cell(%String.prototype%)`, and the hit test is "receiver is a string and `shape_of(holder) == w3_shape`". `length` on a string and index reads keep their existing in-arm paths. Number and boolean receivers stay uncached.

**`GetField2`.** The method-call sequence (`GetField2` then `CallMethod`) lowers to a `GetField` whose destination is the callee temporary, so it shares this cache; there is no separate call-site cache in phase 3.

### 2.3 Named writes: `SetField`

Entry: `shape_before`, `shape_after` (equal to `shape_before` for a write to an existing own data property; a transition child for an add), `slot`, `cell` (guards that no prototype has a setter or a read-only property of that name: it is the receiver's chain cell at fill time).

Hit path: `shape_of(receiver) == shape_before` and `cell_is_valid(cell)`; if `shape_after == shape_before`, `set_slot`; otherwise, if `slot < slots(receiver).len`, write the slot and set the header's shape to `shape_after` (no allocation, no safepoint); if the range is full, the slow path grows it and completes the add, and the entry stays (the next receiver of that shape is sized by the constructor cache, section 2.5). Non-writable, accessor, non-extensible and dictionary cases go uncached through `property::set`.

The cell is necessary: `a.x = 1` on a fresh object must still consult a setter or a read-only `x` added to `Object.prototype` after the entry was filled, and the cell flips on exactly that change.

### 2.4 Globals: `GetGlobal`, `GetGlobalOrUndefined`, `SetGlobal`

The global object is in dictionary mode from creation (phase 1), so its shape never identifies its layout. The `Dictionary` gains a `generation: u32`, incremented on every insert, remove, reflag and compaction; a value write does not change it. Entry: `generation`, `slot`, `flags`. Hit: `global_dictionary_generation() == generation` → `get_slot(global, slot)` (or, for `SetGlobal`, `set_slot` when `flags` say writable). The fill caches own data properties of the global object only; a global found on `%Object.prototype%` stays uncached. This is the "no global epoch" of #122: one counter, one object.

`call0` (1.78× QuickJS in phase 2) is one `GetGlobal` per call; the cached form is a compare and a load.

**Script-level `let` and `const`** stay global object properties without a TDZ, as phase 2 left them; the global lexical record is a second dictionary object with the same cache form and is phase 4 work, when Test262 measures it. No V8 v7 program declares a script-level lexical binding.

### 2.5 Construction: `New`

Phase 2 reserves the receiver's slots from the constructor's static `this.x =` count (`CodeUnit::this_slots`) and looks up `prototype` on the constructor uncached on every `new`. Entry: `fn_shape` (the constructor function object's shape), `proto_slot` (the slot of its `prototype` property), `slots` (the learned receiver size).

Hit: callee is a bytecode function and `shape_of(callee) == fn_shape` → `prototype = get_slot(callee, proto_slot)` (read live, so `F.prototype = …`, which DeltaBlue's `inheritsFrom` does, needs no invalidation); if it is an object, the receiver is `alloc_object(Ordinary, prototype, slots)`. Learning: when a frame with the `CONSTRUCT` flag returns, the arm writes `max(slots, shape_count(receiver))` back into the entry, so the second construction reserves what the first one grew to, including properties added by chained constructors (`Super.call(this, …)`) that the static count cannot see. `this_slots` remains the initial value. The receiver's root shape is one `last_root` compare in `root_shape`.

### 2.6 Diagnostics

A Cargo feature `new-core-ic-stats` (CLI feature `oxide-run --ic-stats`) counts, per site kind: monomorphic hits, polymorphic hits, misses that filled, misses that did not (accessor, dictionary, megamorphic), and prints the totals and the ten sites with most misses (unit name, instruction index, key) at exit. It is the new core's counterpart of the current core's `ic_events.py` and is what M5 reads before touching anything. Release builds without the feature carry no counter.

**Acceptance (M1, M2).** M1: the `GetField` cache (own, prototype and string-receiver forms), the store, the site numbering, the validator and disassembler changes, pruning at sweep, with tests for: a hit after fill; a miss and refill after the receiver's shape changes; a miss after a property is added to, deleted from or reflagged on any prototype of the chain, and after `set_prototype`; a shape id recycled by the sweep not hitting a stale entry (allocate, fill, drop every object of that shape, collect, allocate a new shape, read); the `prop_read` and `proto_read_3` slopes recorded; the hit-path instruction count read from the release assembly as phase 1 M3 did. M2: the `SetField`, global and constructor caches, polymorphic entries, accessor properties, `ic-stats`, with the mirror tests (a transition hit, a full-range miss that still completes, a setter added to `Object.prototype` after fill, a `generation` bump after `delete globalThis.x`, a learned `slots` after a chained constructor) and the `prop_write`, `call0`, `constructor` and `literal` slopes recorded. Both under `gc-stress`.

## 3. The heap

- **Accessor properties.** `PropertyFlags::ACCESSOR` exists; phase 3 gives it storage: an accessor occupies two consecutive slots (getter, setter; `Value::UNDEFINED` for an absent half), `add_property` with the flag reserves them, `lookup_own` reports the first. The slow paths of `GetField`, `SetField`, `GetElem`, `SetElem` and `In` call the getter or setter through the existing re-entrant `call`. No accessor is cached (section 2.2). Builtins use them for `RegExp.prototype.source`, `global`, `ignoreCase`, `multiline` and `flags`; user code reaches them through the `DefineGetter`/`DefineSetter` literal opcodes, which the lowering now accepts.
- **Object kinds.** `Arguments` (payload: an `ArgumentsRecord { cells: Range, mapped: u32 }` arena index; section 5.1), `RegExp` (payload: an index into `Runtime.regexps`, the compiled-program store; `lastIndex` is an own data property), `ForInIterator` (payload: a record with the key snapshot as a value-heap range of strings, a cursor and the object; section 5.4), and `Error` as a flag on `Ordinary` so `ops::is_error` stops walking the prototype chain. `Native` remains the placeholder for phase 4's kinds.
- **Dense arrays.** `GetElem` and `SetElem` already take the dense path for in-range indices; `SetElem` one past the length appends through `set_element`'s growth (Crypto and NavierStokes fill arrays by index, Splay through `push`). Two additions: `new Array(n)` allocates the element range at `n` (holes) rather than growing from 4, and `set_element` growth past 1,024 elements grows by 1.5× rather than 2× (NavierStokes allocates 100-kB arrays; the value heap's power-of-two classes round each range up, phase 1 §M5). Holes stay `Value::EMPTY`; a read of a hole goes to the prototype walk as today.
- **Frames.** `Frame` gains `argc: u16` in its padding (32 bytes stay 32): `arguments`, rest parameters and the learned constructor size need the actual count after `enter` has filled the missing parameters.

## 4. The loop

The additions to `vm::run` are the inline hit paths of section 2 and the in-loop forms of `apply`, `call` and spread (section 5.2). Everything else is a slow path behind `#[inline(never)]`. The size gate of phase 2 (Table C) continues: `vm::run` is measured in every milestone's pull request and stays under 16 KB (12.7 KB at the end of phase 2); a milestone that crosses it moves code out of line before it merges.

## 5. Language surface

### 5.1 `arguments` and rest parameters

The compiler already decides `Arguments(Mapped | Unmapped)` and `Rest(start)`; both are declined today.

- **Unmapped** (strict functions, arrow-free): `Arguments a` allocates an `Arguments` object whose element range holds copies of registers `base + 1 .. base + 1 + argc`, with own data properties `length` and (sloppy only) `callee`. Element reads and writes take the ordinary dense paths; `arguments.length` is a named read with its own cache.
- **Mapped** (sloppy functions with simple parameter lists): the mapping between `arguments[i]` and parameter `i` is two-way, and a register cannot be aliased, so the lowering turns every formal parameter of such a function into a cell at entry (the mechanism phase 2 M2 already uses for captured bindings: `NewCell` per parameter, `LoadCell`/`StoreCell` at each use). The `Arguments` object's record keeps those cells in `cells` and `mapped = min(argc, formals)`; its element range holds the unmapped extras and `Value::EMPTY` at the mapped indices, so the in-arm dense fast paths miss there and `property::get`/`set` on an `Arguments` receiver read or write through the cell. Only functions that use `arguments` pay, which is QuickJS's cost model too (its mapped object holds `var_ref`s to the argument slots). `delete arguments[i]` and `Object.defineProperty` on a mapped index unmap by writing the cell's current value into the element and clearing the cell; the full exotic `[[DefineOwnProperty]]` is phase 4's Test262 work.
- **Rest**: `Rest a, imm = start` allocates an `Array` from registers `base + 1 + start .. base + 1 + argc` (the harness's `(...a) =>` shim, never called, but it must lower for every program to run).

RayTrace constructs every object through `this.initialize.apply(this, arguments)`; with the two items above and section 5.2 that is: one `Arguments` allocation (one header, one record, no copies for mapped parameters), one in-loop frame push.

### 5.2 `apply`, `call` and spread inside the loop

`Function.prototype.apply` and `call` are natives, but a native that calls back into JavaScript is phase 4's machinery. The loop instead rewrites the call site and re-dispatches:

- In the `Call`/`CallMethod` arm, a callee of `Native::FunctionCall` with receiver `f` at `b` and arguments from `b + 2` (`thisArg`, then the real arguments) is exactly the layout of a `CallMethod` on `f` whose window starts at `b + 2`: the arm substitutes `f` as the callee, `b + 2` as the window and `argc - 1` as the count, and falls through to the ordinary dispatch on `f`'s kind (bytecode frame push, or a native with a `CallSite`). No value moves.
- `Native::FunctionApply` with `thisArg` at `b + 2` and an array-like at `b + 3`: if the array-like is a dense `Array` or an `Arguments` object, the arm reserves `b + 3 + length` registers, copies the elements (through the cells for mapped indices) into `b + 3 …`, and dispatches as above with `argc = length`. Any other array-like (an object with `length`) takes the slow path through `property::get` per index, then dispatches the same way. `"".concat.apply("", a)` in Earley-Boyer is this path with a native callee.
- Spread calls: `Apply(Call | Construct)` lower to `CallSpread a, b` and `NewSpread a, b` with the same expansion; `Append` (a spread element in an array literal or argument list) lowers to `AppendSpread a, b`, which appends a dense `Array` or `Arguments` object's elements and throws `InternalError: unsupported: iterator spread` for anything else until phase 4's iterator protocol. `CopyDataProperties` (object spread) stays declined.

A `Call` arm that grows by these three cases is the loop-size risk of section 11; the expansion code is one out-of-line function and only the dispatch after it is inline.

### 5.3 Regular-expression literals

`RegExp(index)` lowers to `NewRegExp a, imm = const`, where the constant is the `(source, flags)` pair; every evaluation allocates a new `RegExp` object, as the specification requires, over one compiled program per literal site, compiled on first evaluation and held in `Runtime.regexps: Vec<CompiledRegExp>` (append-only, like code units; `new RegExp(src, flags)` appends too, deduplicated by `(source, flags)` so Earley-Boyer's per-call `new RegExp` does not grow it without bound). `lastIndex` is an own writable data property; `source`, `flags` and the flag booleans are accessors on `%RegExp.prototype%`.

### 5.4 `for-in`

`ForInStart` lowers to `ForInStart a, b`: a `ForInIterator` over `b` whose record snapshots the enumerable string keys in specification order (own index keys ascending, then own named keys in insertion order, then each prototype's, skipping keys already seen and symbols), with the object. `ForInNext a, b, imm`: `a` receives the next key whose property still exists (QuickJS's has-property check on keys deleted during iteration), or jumps to `imm` when exhausted. The lowering reads the stack code's `ForInNext` plus its consuming jump, as it fuses compare-jumps. `null` and `undefined` sources iterate nothing.

### 5.5 Coverage

Lowered in phase 3 beyond phase 2: `arguments` (both kinds), rest parameters, spread of arrays and `arguments` objects in calls, `new` and array literals, `apply` and `call`, `for-in`, regular-expression literals, `DefineGetter`/`DefineSetter` in object literals, `new.target` (`PushNewTarget` reads the frame's `CONSTRUCT` flag and the callee; it is cheap and `Class.create` idioms probe it). Still declined, with the phase 4 pointer in the message: classes, `super`, private names, generators, async, `await`, `yield`, destructuring, `for-of`, iterators, template literals, object spread, object-literal methods, `eval`, `new Function`, modules, `with` (dropped, phase 0).

**Acceptance (M3).** The two-core corpus (`core/tests/corpus`) gains one program per item: mapped and unmapped `arguments` (reads, writes in both directions, `length`, extras, `delete`), rest parameters, `apply` with arrays, `arguments` objects and array-likes, `call`, spread in calls, `new` and literals, `for-in` over arrays, plain objects, prototype chains, deleted keys and dictionary objects, regular-expression literals (identity per evaluation, `lastIndex`), getters and setters in literals, `new.target`. Each prints identically on both cores with and without stress. The `lowering_census` over the baseline's suite build lowers all eight V8 v7 programs completely (the three Octane and AWFY declines that are phase 4's are listed, not fixed).

## 6. Builtins

### 6.1 The regular-expression engine becomes a crate

`src/regexp` (6,679 lines: compiler, executor, flags, group names, opcodes) depends on the old core through `JsString` in 17 places and on `src/source/unicode` for case folding; `String.prototype.toLowerCase` and `toUpperCase` need the same Unicode tables. Both move into a workspace crate `regexp/` (package `oxide-regexp`, no dependencies, `#![forbid(unsafe_code)]`) whose API takes `&[u16]`; the root crate re-exports it where its current paths are used, so the current engine's behavior cannot change, and `oxide-core` depends on it. The alternative, a regular-expression trait injected into `oxide-core` by the root crate at runtime creation, was rejected: it leaves the Unicode tables behind for phase 4 to move anyway, and it puts a `Box<dyn>` into the runtime for no gain. This pull request moves code the current engine runs, so it is gated by the Test262 focused and full runs besides the workspace tests, and it is the only phase 3 pull request that is.

### 6.2 The realm

The realm gains, as leaf natives in `core/src/builtins/`, exactly what the census of section 0 needs plus the members a program reaches through them (a constructor's `prototype`, a prototype's `constructor`, `Object.prototype.toString` and `valueOf` behind the `ToPrimitive` stand-ins phase 2 hard-coded, which are removed):

| Object | Members in phase 3 |
| --- | --- |
| `globalThis` | `parseInt`, `parseFloat`, `isNaN`, `isFinite`, the constructors below, the error constructors (phase 2) |
| `Object` | callable and constructible (`Object()`, `new Object()`); `prototype`: `toString`, `valueOf`, `hasOwnProperty`, `constructor` |
| `Function` | `prototype` (callable, returns `undefined`): `call`, `apply` (section 5.2), `toString` (the source text is phase 4's; returns `function name() { [bytecode] }`); `new Function` throws `InternalError: unsupported` until phase 4 |
| `Array` | `Array(n)`, `Array(a, b, …)`, `new` forms; `isArray`; `prototype`: `push`, `pop`, `join`, `slice`, `concat`, `indexOf`, `toString` |
| `String` | `String(v)`; `fromCharCode`; `prototype`: `charAt`, `charCodeAt`, `indexOf`, `lastIndexOf`, `slice`, `substring`, `substr`, `concat`, `split`, `replace`, `match`, `toLowerCase`, `toUpperCase`, `toString`, `valueOf`; `new String` throws until phase 4 |
| `Number` | `Number(v)`; `prototype`: `toString(radix)`, `valueOf` |
| `Boolean` | `Boolean(v)`; `prototype`: `toString`, `valueOf` |
| `Math` | `abs`, `floor`, `ceil`, `round`, `min`, `max`, `pow`, `sqrt`, `log`, `exp`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `random`, and the constants `E`, `LN2`, `LN10`, `LOG2E`, `LOG10E`, `PI`, `SQRT1_2`, `SQRT2` |
| `RegExp` | `RegExp(pattern, flags)` and `new`; `prototype`: `exec`, `test`, `toString`, the accessors `source`, `flags`, `global`, `ignoreCase`, `multiline` |
| `Error` family | phase 2's constructors; `prototype.toString` |

Rules for every native: it reads its arguments through `CallSite` by index; a conversion that may run JavaScript (`ToString` of an object, `ToNumber` through `valueOf`) uses the existing re-entrant `call`; a native that needs a value across an allocation roots it (the phase 1 rule, enforced by `gc-stress`). `String.prototype.replace`, `split` and `match` dispatch on the `RegExp` kind directly: the `Symbol.replace` protocol and `Symbol.species` are phase 4 and dropped respectively, so a regular-expression argument runs the regular-expression algorithm and anything else is converted to a string. `replace` with a string replacement implements `GetSubstitution` (`$&`, `$1`, `$$`, `$<`); a function replacement throws `InternalError: unsupported` until phase 4. `toLowerCase`/`toUpperCase` take an ASCII path and fall back to the crate's tables. `Math.random` is QuickJS's xorshift so a program that forgets to replace it still prints the same line; the harness replaces it anyway. `Number.prototype.toString(radix)` and `parseInt` port the current `builtins/primitive.rs` algorithms. Strings stay flat: `substring`, `slice` and `concat` allocate (section 11 has the rope trigger).

Natives are a `Native` enum with one variant per member (about 80) indexed from the function record as today; a function object made by `native_function` gets no `name` or `length` in phase 3 (phase 4, with the embedder API).

**Acceptance (M4).** The regular-expression move: workspace tests, Test262 focused and full unchanged, `cargo tree -p oxide-core` shows `oxide-regexp` as its only dependency. The builtins: one corpus program per table row above exercising every listed member, including the edge cases the current `builtins` tests cover at the leaf level (negative and fractional indices, radix bounds, `NaN` and `±0` through `Math`, empty-string splits, `lastIndex` with and without `g`, replacement patterns), printing identically on both cores with and without stress; the eight V8 v7 programs run to their completion line on `oxide-run` and print the manifest's exact stdout.

## 7. The driver and CI

`oxide-run` gains `--ic-stats` (section 2.6, feature-gated) and nothing else. `fixed.py` runs it as before; the eight programs are the baseline's `suite_matrix.py build` output and its manifest (sha256 `2f775c66ae9838db…`), which stay outside the repository.

CI, in the `fast` job: the enlarged corpus under `check-new-core-stress.py` as today. New, as a manual and nightly `workflow_dispatch` tier because the suite build needs the pinned upstream checkouts: the eight programs at one `run()` per benchmark through `oxide-run --gc-stress=1` built with the stress feature, their stdout compared to a one-run manifest that `suite_matrix.py build --runs 1` writes. That is phase 0 §3's "the V8 v7 programs at one `run()` per benchmark under `--gc-stress=1`", and it gates every change to the heap, the loop, the caches and the natives from M4 on.

## 8. Milestones

| # | Content | Acceptance | Estimate |
| --- | --- | --- | --- |
| M1 | `IcStore`, site numbering, validator and disassembler, pruning at sweep and the stress verifier check, the `GetField` cache (own, prototype, string receiver), monomorphic | section 2 (M1 list); `prop_read` and `proto_read_3` slopes; hit-path assembly count | 3–4 days |
| M2 | `SetField`, global and constructor caches; polymorphic entries; accessor properties; `ic-stats` | section 2 (M2 list); `prop_write`, `call0`, `constructor`, `literal` slopes | 4–6 days |
| M3 | `arguments` (mapped through parameter cells, unmapped), rest, `apply`/`call`/spread in the loop, `for-in`, regular-expression literals, getters and setters in literals, `new.target`, `Frame::argc` | section 5.5 corpus on both cores; the eight programs lower completely | 5–7 days |
| M4 | `oxide-regexp` (one pull request, Test262-gated); the realm of section 6.2 (a second) | section 6.2; the eight programs print their completion line | 7–10 days |
| M5 | profile and tune: `ic-stats` and `perf record` on the eight; the open decisions of section 10 assigned here; the GC trigger on Splay | every tuning change justified by a before/after slope or whole-program count in the pull request | 4–6 days |
| M6 | reference-machine measurement and the go/no-go record | the tables of section 9 and the decision committed under `docs/performance` | 2 days |

Total: about 25–35 working days. M1 and M2 may be one pull request if the `SetField` cache is needed to test the transition path of the constructor cache; M3 and M4 are always separate, and the regular-expression move is always its own pull request.

## 9. Measurement, exit and the go/no-go

Every number is produced as in phases 1 and 2: the [baseline](baseline.md)'s machine, binaries (its `qjs` sha256 `3cc9c95d597af1bd…`, its QuickJS `3fd16c25ae63a7cb…`) and manifest, `fixed.py --repeat 5 --cpu 2 --linux-counters --timeout 900`, the three engines interleaved in rotating order, medians of 5, every sample's stdout checked, raw samples kept outside the repository. Slopes for the probes come from `probes/new-core/phase2.py` as before.

**Table A: the eight V8 v7 programs.** Whole-process time and `instructions:u` for QuickJS, the current core and the new core; the two ratios per program and the geometric means. The gate is read from this table. For orientation, the baseline's current-core figures and what a 3× gain means per program:

| Program | QuickJS ms | Current ms | Current / QuickJS | New core at 3× (ms) | …as a multiple of QuickJS |
| --- | ---: | ---: | ---: | ---: | ---: |
| richards | 1,065 | 4,169 | 3.92 | 1,390 | 1.30 |
| deltablue | 1,092 | 4,765 | 4.36 | 1,588 | 1.45 |
| crypto | 981 | 3,448 | 3.52 | 1,149 | 1.17 |
| raytrace | 1,083 | 5,073 | 4.68 | 1,691 | 1.56 |
| earley-boyer | 1,030 | 4,457 | 4.33 | 1,486 | 1.44 |
| regexp | 1,192 | 1,790 | 1.50 | 597 | 0.50 |
| splay | 1,766 | 5,562 | 3.15 | 1,854 | 1.05 |
| navier-stokes | 1,050 | 3,090 | 2.94 | 1,030 | 0.98 |
| geometric mean | | | 3.38 | | 1.13 |

The current core is 3.38× QuickJS's time and 3.97× its instructions over the eight, so "3× the current engine" means the new core at about 1.13× QuickJS's time, which is where the fixed probes already are (1.12×). RegExp cannot contribute 3×: its time is in the regular-expression engine both cores share, and the 1.5× gap is the interpreter around it. If RegExp reaches 1.5× and the gate is still to hold, the other seven must average 3.3×.

**Table B: the fixed probes and the phase 1 workloads**, as phase 2's Tables A and B, so the cached paths are compared with the uncached ones construct by construct: `prop_read`, `prop_write`, `array_read`, `array_write`, `call0`, `proto_read_3`, `literal`, `constructor`, with the IC hit-path instruction counts from the assembly beside them.

**Table C: the loop and the caches.** `vm::run`'s size against the 16 KB gate; `ic-stats` per program (hit rate per cache kind, polymorphic and megamorphic site counts, the ten worst sites); the layout spread of #119's experiment is not repeated unless Table A's medians disagree with the probe slopes.

**Table D: the collector on the eight.** Collections, total pause, p50/p99/max pause, peak RSS and heap reserved per program at the default trigger; Splay is the program that exercises it. The trigger policy (4 MiB minimum, growth 2) is tuned in M5 only if this table shows the collector above 10% of any program's time, and the tuning is recorded here.

**Exit condition and the go/no-go.** Tables A–D committed under `docs/performance`; `gc-stress` green on everything in section 7, including the eight programs at one `run()`; the current engine's Test262 unchanged (focused 6,844 of 6,844; full 83,304 of 83,354 as frozen). The gate of #122 is then read from Table A as follows:

- **Go**: the geometric mean over the eight of (current-core time / new-core time) is at least 3.0, and no program other than RegExp is below 2.5. Phase 4 starts on the design as built.
- **No-go**: either condition fails. #122 says "re-plan here instead of starting phase 4": the re-plan is written against Table A's per-program split and Table C's hit rates, not against the geometric mean, so it names the mechanism that fell short. Nothing is deleted or reverted by a no-go; the measurement stands.

The per-program floor is this page's addition to #122's one-line gate, following the project rule that no single-case regression hides behind a geometric mean; the maintainer confirms or changes it before M6 runs. Instruction ratios are reported next to time ratios but do not decide the gate, because the probes already showed the new core retiring more instructions per unit time than QuickJS and the gate is about user-visible time.

## 10. Decisions

Made, and not reopened inside phase 3:

- caches live in an `IcStore` beside the code store, indexed per unit from a free operand (`c`, `b | c << 8`, or `imm`); no encoding change, no cache in the heap;
- entries are weak: pruned at sweep against the mark bits, verified under `gc-stress`;
- four cache kinds: read (own, prototype, string receiver), write (in place and transition, cell-guarded), global (dictionary generation), construct (function shape, live `prototype` slot, learned size); no accessor, element or call-site cache;
- monomorphic inline, up to four entries out of line, megamorphic stops recording;
- accessors are two slots behind one `ACCESSOR` flag;
- mapped `arguments` aliases parameters through cells decided in the lowering; no compiler change;
- `apply`, `call` and spread rewrite the call site inside the loop; the Step/Resume native ABI stays phase 4's;
- the regular-expression engine and the Unicode tables become `oxide-regexp`, the one dependency of `oxide-core`;
- typed arrays, ropes, the global lexical environment, wrapper objects and function `name`/`length` are phase 4;
- the go/no-go is read from Table A with the floor of section 9.

Open, with the milestone that decides them:

- a complete-miss (`undefined`) cache for `GetField`: M5, by `ic-stats` on RayTrace and Earley-Boyer;
- whether a megamorphic site re-records after a cooldown: M5, by the megamorphic site counts;
- a lazily created `prototype` object for closures (QuickJS's auto-init property) instead of one per `MakeClosure` of a `function`: M5, by the `MakeClosure` share in the profile of Splay and Earley-Boyer;
- a cached `prototype` slot for `instanceof` (Earley-Boyer's 70 sites): M5, by the profile;
- the GC trigger parameters: M5, by Table D;
- the `bool` fast path for `JTrue`/`JFalse` on `In`/`InstanceOf` results fused with their jump: M5, by the profile;
- whether `Frame` stays at 32 bytes when `argc` and a later `new.target` slot are both needed: M3.

## 11. Risks

- **Stale cache entries.** Recycled shape, cell and object ids are the one way a cache can read the wrong slot. Mitigations: pruning at sweep is part of M1, not a later fix; the stress verifier checks every filled entry after every collection; the recycled-id test of M1 is written before the hit path is.
- **Mapped `arguments`.** The aliasing is the semantic trap of this phase. Mitigation: the lowering-only cell rule keeps the compiler untouched, and the M3 corpus tests writes in both directions, extras, `delete` and `apply` before any measurement.
- **Loop size creep.** `apply`, `call`, spread and four hit paths all want to be inline. The 16 KB gate is checked in every pull request; expansion code is out of line from the first commit.
- **The gate's arithmetic.** RegExp dilutes the geometric mean by construction, so the seven others need about 3.3× each. If M5's profile shows the remaining gap in the interpreter header (phase 2's 19 instructions per dispatch) rather than in the paths this phase builds, that is a finding for the re-plan, not something M5 should try to fix.
- **Flat strings.** `+=` loops and repeated `substring`/`slice` are quadratic without ropes. Trigger: if any of the eight programs spends 5% or more of its instructions in string allocation or copying in M5's profile, the one-byte/two-byte rope of phase 4 is pulled forward into M5 as its own pull request; otherwise it waits.
- **The collector on Splay.** A 4 MiB minimum heap collects often on a program whose live set is tens of megabytes. Table D exists to see it; the growth factor is the knob.
- **The regular-expression move.** It touches code the current engine runs (the lexer's identifier tables move too). Mitigation: a pure move with re-exports, no behavior change, Test262 focused and full on that pull request.
- **`Math` parity.** `Math.pow`, `log`, `exp` and the trigonometric functions call the platform libm through Rust's `f64` methods, as the current core does; results and stdout match QuickJS on the reference machine because both use glibc. Other platforms were not part of the baseline and are not part of the gate.

## 12. Seam with phase 4 (preview, not phase 3 work)

Phase 4 ports the remaining builtins as Step/Resume machines over the native ABI, which phase 3 has not touched: natives stay leaf functions, and the two that run JavaScript do it by frame rewriting. The cache store takes an accessor entry kind and an element kind without changing its layout; the `Arguments` record takes the exotic `[[DefineOwnProperty]]`; `Runtime.regexps` and `IcStore` are the second and third append-only stores whose collection is decided together with code units; ropes replace the flat concatenation in `ops::concat` behind the same interface; the global lexical record reuses the generation-and-slot cache; the typed-array fast path lands in the `GetElem` arm behind the kind check that the `Arguments` kind already forces there.
