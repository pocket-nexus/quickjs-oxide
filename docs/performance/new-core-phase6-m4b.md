# New core phase 6, M4b: the spec alignment

The implementation plan of [phase 6](new-core-phase6.md)'s M4b
([section 5.1](new-core-phase6.md#51-the-source-of-truth-amended-after-m3)):
ECMA-262 and Test262 decide correctness, QuickJS 2026-06-04 is the reference
only where the specification leaves a behavior to the implementation or the
host. M4b runs after M4's deletion and lands in pull request #152.

It does two things: it returns every behavior M3 copied from QuickJS where
QuickJS departs from the specification, and it lifts the register lowering's
256-register operand limit.

## 1. Goals and acceptance

- Every item of section 3 is fixed or, after a clause-by-clause check, kept
  with the clause that leaves it open written down.
- No Test262 variant that passes on M2 fails, except the four direct-eval
  variants of `NEWCORE-DIRECT-EVAL-001`; the seven M3 lost to QuickJS
  behaviors pass again.
- A function needs no more than its register window: calls with up to 65,534
  arguments, templates up to the front end's QuickJS stack bound, and nesting
  up to the parser's limits compile and run; the five tests ignored in M3 run.
- Every oracle case or fixture whose QuickJS output departs from the
  specification is attributed `QUICKJS-SPEC-NNN` and must still differ.
- Gates: clippy, fmt, the workspace tests, `oxide-core --features gc-stress`,
  `check-new-core-stress.py`, the oracle corpus, the fixtures, the C oracles,
  the parity slice; the full Test262 receipt re-frozen once at the end; the
  eight V8 v7 programs within 2% of M4's `qjs` in `instructions:u`; `run_at`
  under 16,384 bytes.

## 2. Tools set up first

1. **Baselines kept.** At M4's last commit: the full Test262 TSV and JSONL
   (`target/test262-full.*`, copied out of `target/`), M4's `qjs`, and the
   register disassembly of the 37 suite programs and the 20 probes
   (`qjs --disassemble`), so that M4b's diffs need no rebuild of an old tree.
2. **A three-way differential.** `scripts/checks/spec-triage.py`: every
   oracle case is run on `qjs`, the pinned QuickJS and Node 24 (V8, a third
   implementation). Print-mode cases get a `print` shim; completion modes run
   through `vm.runInThisContext`. Error messages, `stack` and the printer
   are normalized away (they are implementation-defined): a thrown error
   compares by its constructor name only. A case where QuickJS and Node
   disagree is a candidate departure; each candidate is decided by the
   specification clause, never by majority.
3. **The registry of QuickJS departures.**
   `dev-support/quickjs-spec-departures.tsv`: `id`, `clause` (ECMA-262
   section and algorithm name), `summary`, `test262` (paths or `-`).
   `oracle-attributions.tsv` and the fixture gates' `deviation` column accept
   these identifiers; `check-oracle-registry.py` checks that every
   `QUICKJS-SPEC-NNN` it meets is in the registry and every registry row is
   used.

## 3. The items

Clauses are cited by number and algorithm name (ECMA-262 2025); the algorithm name is authoritative. Each item lists the clause, what M3 does, the change, and its tests. "Test"
means an oracle case (attributed `QUICKJS-SPEC-NNN` when QuickJS differs) or
a Rust test where the behavior has no JavaScript surface; Test262 paths are
named where they exist.

### 3.1 Confirmed departures

1. **BigInt `--x`** (13.4.5.1 Postfix Decrement, BigInt::subtract): M3 adds
   2³² − 1 outside `int64`. Change: `Unary::Dec` is `qjs::add(&value, &one,
   true)` for every value (`core/src/vm/ops.rs`). Test: `--x` and `x--` at
   `±2⁶³`, `±2⁶⁴`, `2¹⁰⁰`; the update-matrix test's two QuickJS rows become
   spec rows.
2. **A bound function's `[[Prototype]]`** (10.4.1.3 BoundFunctionCreate,
   step 1): `? targetFunction.[[GetPrototypeOf]]()`. M3 uses
   `%Function.prototype%`. Change: `builtins/function.rs::bind` reads the
   target's prototype through `object::get_prototype_of` (a proxy target runs
   its trap; its throw propagates before the bound function is made).
   Test262: `staging/sm/class/boundFunctionSubclassing.js` ×2.
3. **`Function.prototype.toString` without source** (20.2.3.5, the
   NativeFunction grammar): the text must parse as
   `function NativeFunctionAccessor? PropertyName? (FormalParameters) { [native code] }`.
   M3 prints `function bound f() {…}`. Change: the name is printed only when
   it is a PropertyName (an IdentifierName, or a `[Symbol.x]` computed name
   as built-in symbol-named methods have) after an optional `get `/`set `
   accessor prefix; otherwise none (`function () {\n    [native code]\n}`).
   Test262: `staging/sm/Function/function-bind.js`,
   `built-ins/Function/prototype/toString/*`.
4. **`Object.defineProperties`** (20.1.2.3.1 ObjectDefineProperties): keys
   from `props.[[OwnPropertyKeys]]()`, then for each key
   `[[GetOwnProperty]]`, and if present and enumerable `Get` and
   `ToPropertyDescriptor` into a list; only then `DefinePropertyOrThrow` for
   each. M3 reads all keys' descriptors first and defines while reading.
   Change: `builtins/object.rs::define_properties_from` follows the steps
   (the list rooted). Test262: `staging/sm/object/defineProperties-order.js`
   ×2. The Rust test that pins QuickJS's batch order checks the spec order.
5. **Proxy `[[GetOwnProperty]]` invariants** (10.5.5, steps 11–17):
   `IsCompatiblePropertyDescriptor`, and the two "non-configurable" checks
   (step 17: a non-configurable, non-writable result requires the target's
   property to be non-writable). M3 accepts a frozen property reported with
   another value. Change: `vm/proxy.rs::get_own_property` calls the same
   `ValidateAndApplyPropertyDescriptor(undefined, …)` comparison as
   `[[DefineOwnProperty]]` (value `SameValue` for a non-configurable,
   non-writable data property). Test262:
   `staging/sm/regress/regress-1383630.js` ×2. Section 3.2.4 audits the
   other twelve traps.
6. **`Promise.allSettled`'s element functions** (27.2.4.2.1 step 8.l–8.s):
   `onFulfilled` and `onRejected` share one `alreadyCalled` record. Change:
   `builtins/promise.rs` builds the rejected half over the fulfilled half's
   record (as before M3). Test: a thenable calling both halves keeps the
   first.
7. **The combinators' result** (27.2.4.1.2, 27.2.4.2.2, 27.2.4.3.2, and the
   resolve-element functions): `values` and `errors` are Lists; the array is
   `CreateArrayFromList` when the count reaches zero. M3 passes a live array
   created up front and defines into it (a frozen array swallows writes).
   Change: the shared state keeps the elements in an internal record that no
   script sees; at zero a fresh array (or `AggregateError`'s `errors`) is
   made from it. Test: a resolve that freezes or mutates the array it gets,
   and a late element call after resolution, see a fresh array.
8. **`#x in o` for a setter-only accessor** (13.10.1, `PrivateIdentifier in
   ShiftExpression`; 7.3.31 PrivateElementFind): true iff `o` carries the
   class's brand. M3 looks for an own property named `"[unsupported
   type]"`. Change: the lowering of `PrivateIn` for a `PrivateSetter` binding
   loads the setter (the binding that `CallPrivateSetter` reads) and emits
   `PrivateBrandIn` on it; `vm/class.rs::unbranded_in` is deleted. Test: the
   r3j fixture rows and a setter-only/getter-only/both matrix.

### 3.2 Changes to check clause by clause

Each is checked against the clause first; the column says what the check is
expected to find, which the check confirms or overturns.

#### 3.2.1 Check orders and error kinds

| Behavior | Clause | Expected |
| --- | --- | --- |
| Atomics: kind check before detached/out-of-bounds | 25.4.3.1 ValidateIntegerTypedArray: ValidateTypedArray (detached or out of bounds: `TypeError`) first | revert |
| Atomics on an array out of bounds after a resize: `RangeError` | ValidateTypedArray throws `TypeError` | revert |
| `Atomics.wait`: SharedArrayBuffer check before detached | 25.4.11 DoWait: ValidateIntegerTypedArray first | revert |
| Atomics rechecks after `ToIndex` (all but `notify`) | AtomicReadModifyWrite, `Atomics.load`/`store`/`compareExchange`: RevalidateAtomicAccess; `notify` has none | keep |
| `Object.groupBy`/`Map.groupBy`: callback before items | 7.3.35 GroupBy: RequireObjectCoercible(items), then IsCallable(callback) | revert |
| `%TypedArray%.from`: mapfn before constructor, constructor after the source | 23.2.2.1: IsConstructor(C), then mapfn | revert |
| `%TypedArray%.prototype.toSorted`: receiver before comparator | 23.2.3.33: comparator first | revert |
| `Reflect.construct`: target, newTarget, list | 28.1.2: target, then newTarget, then CreateListFromArrayLike | confirm |
| `ArrayBuffer.prototype.transfer` past `maxByteLength`: `TypeError` | 25.1.3.3 ArrayBufferCopyAndDetach: `RangeError` | revert |
| DataView get/set after a resize | 25.3.1.5 GetViewValue: IsViewOutOfBounds → `TypeError`, then `getIndex + size > viewSize` → `RangeError` | align |
| string past the length limit: `InternalError` | implementation limit (6.1.4 sets 2⁵³ − 1; anything lower is a resource limit) | keep |

#### 3.2.2 ArrayBuffer

| Behavior | Clause | Expected |
| --- | --- | --- |
| `length`, `maxByteLength`, `resize`/`grow` through `ToInt64` | the `ArrayBuffer` constructor, GetArrayBufferMaxByteLengthOption, `ArrayBuffer.prototype.resize`: `ToIndex` (`RangeError` for negative) | revert |
| `maxByteLength` of a detached buffer keeps its value | 25.1.6.4: 0 when detached | revert |
| `resize`/`grow` convert before the detached check | 25.1.6.6 steps 4–5 | keep |

#### 3.2.3 Arrays and iteration

| Behavior | Clause | Expected |
| --- | --- | --- |
| `sort`: no write for an element already in place | 23.1.3.30 step 7: `Set` for every sorted index, `DeletePropertyOrThrow` for the holes | revert |
| `sort`: comparator call sequence, `ToString` when needed | SortIndexedProperties: an implementation-defined sequence of SortCompare calls | keep |
| `find`/`findIndex`…: receiver passed unwrapped | 23.1.3.9: `O = ToObject(this)` is the callback's third argument | revert |
| change-by-copy limit at 2³¹ − 1 | ArrayCreate allows 2³² − 1; a lower bound is a resource limit, but the `TypeError` past 2⁵³ − 1 must come first | check against `built-ins/Array/prototype/{toSpliced,with,toReversed}` |
| spread reads `@@iterator` twice | 13.3.8.1 ArgumentListEvaluation → GetIterator: once | revert |
| spread's fast array copy | allowed only while unobservable: `%Array.prototype%[@@iterator]` and `%ArrayIteratorPrototype%.next` unmodified | confirm the guard |
| spread and `AggregateError` close the iterator on any throw | IteratorStep/IteratorValue throws do not close (7.4.x); only a throw from elsewhere does | revert |
| `for await`: `value` read when done; a completed iterator closed | 14.7.5.6 ForIn/OfBodyEvaluation: done → return without reading `value` or closing | revert |
| for-in rewrite | 14.7.5.9 EnumerateObjectProperties: implementation-defined within its constraints | confirm the constraints |
| iterator helpers, `Iterator.from`, `Iterator.concat` state machines | 27.1.2.x: the closure-based algorithms | check against `built-ins/Iterator/**` and the three-way differential |
| async generator: one trailing request per driver entry | 27.6.3.x AsyncGeneratorDrainQueue: drains until the queue is empty or a request suspends | check; revert if it differs |

#### 3.2.4 Proxy

The rewrite after `js_proxy_*` is compared trap by trap with 10.5.1–10.5.13
(each invariant step), with `built-ins/Proxy/**` and the three-way
differential; 3.1.5 is the first finding.

#### 3.2.5 Language and globals

| Behavior | Clause | Expected |
| --- | --- | --- |
| a read of an uninitialized global lexical outside a lexical reference falls back to the global object | 9.1.1.4.1 HasBinding / 9.1.1.4.6 GetBindingValue: the declarative record first, uninitialized → `ReferenceError` | revert |
| a var kept beside a masked global lexical (Annex B) | B.3.2.2 GlobalDeclarationInstantiation changes | check against `annexB/language/global-code/**` |
| a global declaration checks the global property before the lexical | 16.1.7 GlobalDeclarationInstantiation step order | check |

#### 3.2.6 Numbers, BigInt, intrinsics

| Behavior | Clause | Expected |
| --- | --- | --- |
| `Math.sumPrecise` with QuickJS's `int64` wrap | the exact sum (ES2026 `Math.sumPrecise`) | revert to an exact algorithm |
| `BigInt.asUintN`/`asIntN` with QuickJS's quirks | 21.2.2.1–2: exact modular reduction | revert any quirk |
| BigInt allocation limits | resource limits | keep |
| `BigInt.prototype.toLocaleString` removed | 21.2.3.2 requires it | restore |
| `String.prototype.length` configurable | 22.1.3: a String exotic object's `length` is non-configurable | revert |
| every other attribute, `length` or `name` the shape commit changed | the property tables of clauses 20–28 | diff M2 → M4b descriptors; check each against its clause |
| `Uint8Array.fromHex`/`setFromHex` count UTF-8 bytes | the Base64/hex clauses count code units | check |
| RegExp and String protocol (replace fast paths, `$<name>`, `test` and constructor order, `split`, `match`/`matchAll`/`search`) | 22.2.4.1, 22.2.6.x, 22.1.3.x | check against `built-ins/RegExp/**`, `built-ins/String/**` and the three-way differential; a fast path must be unobservable |

Kept, the specification leaving them open: error messages and `stack`, the
printer, the intrinsics' property order, sort comparison sequence, for-in
order within its constraints, NaN bit patterns, argument and nesting limits,
the async stack-overflow rejection, QuickJS's non-standard
`Function.prototype.fileName`/`lineNumber`/`columnNumber` (no clause forbids
them), the global object's `Symbol.toStringTag` (the global object is the
host's).

## 4. The register window

### 4.1 The problem

The register lowering (`src/engine/compiler/register/mod.rs`) maps operand
stack depth `d` to register `temp_base + d` and requires
`first_local + locals + temps ≤ 256` with the locals spilled
(`Layout::of`); calls also stop at 255 arguments (`call`). A function whose
operand stack is deeper than what the window leaves does not lower.

### 4.2 The design

Temporaries past the window go to the spill scope, as spilled locals
already do; operations reach them through a small pool of scratch
registers.

1. **Layout.** When `first_local + locals + temps > WINDOW`: reserve
   `SCRATCH = 8` registers at the top of the window; keep at most
   `min(locals, WINDOW / 4)` locals in the window; the temporaries that fit,
   `T = WINDOW − first_local − window_locals − 1 (spill) − SCRATCH`, are
   registers; depths `d ≥ T` are spill slots `SPILLED_TEMP + (d − T)`,
   allocated after the spilled locals in the same spill scope
   (`spill_slots` grows by `max_depth − T + 1`). A function that fits keeps
   today's layout, bit for bit.
2. **Access.** Every read of a stack entry goes through `operand(depth)`;
   for `d ≥ T` it emits `LoadSlot scratch, spill, slot` into the next
   scratch register (round robin over `SCRATCH`, never reusing one an
   instruction under construction holds). Every write goes through
   `result_register()` plus `push_result`: for `d ≥ T` the instruction
   writes a scratch register and `push_result` emits `StoreSlot`; the
   `last_dest` retargeting is off for such writes. The 36 direct
   `Self::r8(self.temp(…))` sites become calls to these two helpers.
3. **Fixed windows.** Instructions that read consecutive registers
   (`Call*`, `New*`, `TailCall*`, `NewArray` from `ArrayFrom`, `Apply`, the
   iterator records of `for-of`/`for-await`/destructuring, `IteratorCall`)
   keep today's code when their range is below `T`. When it reaches `T`:
   - the iterator-record family copies the record's three or four entries
     into consecutive scratch registers, runs, and copies back what it
     wrote;
   - a call whose window does not fit in `SCRATCH` (or whose argument count
     passes 255) becomes: callee and receiver into scratch, `NewArray` empty,
     one append per argument (read from its slot), then `CallSpread` /
     `NewSpread` over the array (`CallSpread` copies a dense array without
     the iterator protocol, 5.2 of phase 3, so nothing observable changes);
     a tail call loses its tail form;
   - `ArrayFrom(n)` past `T` becomes `NewArray` empty plus one append per
     element.
4. **Control flow.** Depths are fixed per instruction by the analysis, so a
   label's stack has the same layout on every incoming edge: a spilled depth
   is spilled on all of them.
5. **The front end is unchanged.** Its QuickJS-shaped stack code keeps
   computing QuickJS's compile-time stack bound (65,532 substitutions accept,
   65,533 throws `InternalError: stack overflow`), which the template test
   pins.

### 4.3 Tests

- Lowering unit tests (`register/run_tests.rs`): calls with 254, 255, 256,
  1,000 and 65,534 arguments (the last at the runtime's argument limit); a
  template with 300 and 65,532 substitutions; object, array and call
  nesting at 300 and 1,000 levels; a deep expression containing a `for-of`,
  a destructuring and a `try`; a deep `new` and a deep tail call; spilled
  locals and spilled temporaries in one function; each run under
  `gc-stress`.
- The five ignored tests run (`template_stack_limit…`, four of
  `parser_stack_depth.rs`).
- The 37 suite programs' and 20 probes' disassembly is byte-identical to
  M4's (no function among them passes the window).
- The instruction gate and `run_at`'s size: unchanged (the change is in the
  lowering only).

## 5. Order of work

| Step | Content | Exit |
| --- | --- | --- |
| 0 | the baselines and tools of section 2 | the three-way run over the corpus, its candidate list in the results page |
| 1 | the clause-by-clause table of 3.2 filled in: each row's finding and decision | the table committed before any fix |
| 2 | 3.1 and 3.2's reverts, area by area (Function and Object; Proxy; Promise and class; BigInt, Math and intrinsics; buffers, typed arrays and Atomics; arrays and iteration; language and globals), each with its tests and attributions | the corpus, fixtures and focused Test262 after each area |
| 3 | the register window (section 4) | section 4.3 |
| 4 | the registry, the attributions, the fixture gates, the Rust tests that pinned QuickJS behaviors rewritten to the clause | `check-oracle-registry.py`, the corpus |
| 5 | the full Test262 run, diffed against M4's report row by row; every change explained; the receipt re-frozen once; the metrics regenerated | the acceptance of section 1 |
| 6 | the gates of section 1; the M4b section of the results page | all pass |

Estimate: 6–9 days (steps 0–2: 3–4, step 3: 2–3, steps 4–6: 1–2).

## 6. Risks

- **The bound prototype read** adds a `[[GetPrototypeOf]]` per `bind`;
  `bind` is not on the V8 v7 programs' hot paths. Measured by the gate.
- **Spread back to one `@@iterator` read** may restore an iterator
  allocation QuickJS's double read avoided; the fast path's guard keeps
  arrays allocation-free.
- **The register window** touches the lowering's every stack access. The
  byte-identical disassembly of the suite and probes is the guard that
  functions inside the window are untouched; `gc-stress` covers the new
  spill slots as roots.
- **The three-way differential** may surface departures beyond section 3.
  Each new one is decided by its clause and added to the registry; if the
  list grows past what the estimate allows, the remaining ones are recorded
  and the decision taken with the maintainer.
