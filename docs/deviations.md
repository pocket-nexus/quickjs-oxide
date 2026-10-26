# Deviation ledger

This ledger lists where quickjs-oxide departs from QuickJS 2026-06-04, as
described in [compatibility](compatibility.md). ECMA-262 and Test262 are the
source of truth; QuickJS 2026-06-04 is the reference only where the
specification leaves a behavior to the implementation or the host. A
departure is therefore one of two kinds:

- an **approved target deviation** below: a behavior that differs from both
  QuickJS and, for the `TARGET-*` entries, the specification, by the
  maintainer's decision, or that follows the specification where pinned
  QuickJS fails a Test262 variant;
- a **specification departure of QuickJS**, registered in
  [`quickjs-spec-departures.tsv`](../dev-support/quickjs-spec-departures.tsv)
  as `QUICKJS-SPEC-NNN` (see [below](#quickjs-specification-departures)):
  quickjs-oxide follows the specification clause where QuickJS does not.

Each entry covers exactly the observable behavior and test variants it names;
any other mismatch with QuickJS where the specification leaves the behavior
open, and any mismatch with the specification, is a defect.

## Approved target deviations

The five entries `TARGET-*` record features the maintainer decided on
2026-10-16 the engine does not implement. Their Test262 evidence is
[`dropped-variants.tsv`](../dev-support/test262/dropped-variants.tsv):
the 1,897 variants (D) that pass with the five features and fail without
them, each with its reason. Of these, 27 pass anyway (negative `with` tests
whose expected early error the front end gives either way) and are not
deviations; the other 1,870 fail by decision.

### TARGET-WITH-001

- Status: approved target deviation (2026-10-17).
- Surface: the `with` statement in sloppy code.
- Authority: the maintainer's decision of 2026-10-16.
- Required behavior: `with` is an early `SyntaxError` (`with statements are not
  supported`) in sloppy code, as it is in strict code. Every identifier resolves
  at compile time to a register, a scope slot or a global; no object environment
  enters the scope chain.
- Baseline evidence: [`dropped-variants.tsv`](../dev-support/test262/dropped-variants.tsv)
  lists 311 variants of reason `with`.
- Acceptance: `with ({}) {}` throws `SyntaxError` before any statement of the
  script runs. The variants of reason `with` fail by decision and are excluded
  from the Test262 metric's difference list; any other variant that fails is a
  defect.
- Specification: [The with
  statement](https://tc39.es/ecma262/multipage/ecmascript-language-statements-and-declarations.html#sec-with-statement).

Minimal probe:

```js
with ({ x: 1 }) { print(x); }
```

### TARGET-UNSCOPABLES-001

- Status: approved target deviation (2026-10-17).
- Surface: `Symbol.unscopables`.
- Authority: the maintainer's decision of 2026-10-16.
- Required behavior: `Symbol.unscopables` and
  `Array.prototype[Symbol.unscopables]` exist with their specified values;
  nothing reads them, since no object environment exists to consult them
  (`TARGET-WITH-001`).
- Baseline evidence: [`dropped-variants.tsv`](../dev-support/test262/dropped-variants.tsv)
  lists 53 variants of reason `unscopables`.
- Acceptance: `typeof Symbol.unscopables` is `"symbol"`; the variants that
  observe a lookup through `with` fail with `TARGET-WITH-001`'s `SyntaxError`.
  The variants of reason `unscopables` fail by decision and are excluded from
  the Test262 metric's difference list; any other variant that fails is a
  defect.
- Specification:
  [Symbol.unscopables](https://tc39.es/ecma262/multipage/fundamental-objects.html#sec-symbol.unscopables).

Minimal probe:

```js
print(typeof Symbol.unscopables, Array.prototype[Symbol.unscopables].flat);
```

### TARGET-DIRECT-EVAL-001

- Status: approved target deviation (2026-10-17).
- Surface: direct `eval`.
- Authority: the maintainer's decision of 2026-10-16.
- Required behavior: `eval(src)` at a direct-eval call site runs as indirect
  eval: in the global scope, sloppy unless `src` is strict, with no access to
  the caller's bindings, `this`, `arguments` or `new.target`, and no `var`
  injection into the caller. `new Function` and indirect eval are unchanged.
- Baseline evidence: [`dropped-variants.tsv`](../dev-support/test262/dropped-variants.tsv)
  lists 517 variants of reason `direct-eval`.
- Acceptance: inside `function f() { var x = 2; return eval("x"); }` with a
  global `var x = 1`, `f()` returns `1`. The variants of reason `direct-eval`
  fail by decision and are excluded from the Test262 metric's difference list;
  any other variant that fails is a defect.
- Specification:
  [PerformEval](https://tc39.es/ecma262/multipage/global-object.html#sec-performeval).

Minimal probe:

```js
var x = 1; function f() { var x = 2; return eval('x'); } print(f());
```

### TARGET-REALMS-001

- Status: approved target deviation (2026-10-17).
- Surface: realms.
- Authority: the maintainer's decision of 2026-10-16.
- Required behavior: one realm per runtime. No API creates a second context on a
  runtime (`Context::new` returns the runtime's one realm, and a request for
  another is `RuntimeError::Unsupported`); `$262.createRealm` throws `TypeError:
  realms are not supported`. Isolation uses separate runtimes.
- Baseline evidence: [`dropped-variants.tsv`](../dev-support/test262/dropped-variants.tsv)
  lists 488 variants of reason `realm`.
- Acceptance: `$262.createRealm()` under the Test262 host throws a `TypeError`
  with that message. The variants of reason `realm` fail by decision and are
  excluded from the Test262 metric's difference list; any other variant that
  fails is a defect.
- Specification:
  [Realms](https://tc39.es/ecma262/multipage/executable-code-and-execution-contexts.html#sec-code-realms).

### TARGET-SPECIES-001

- Status: approved target deviation (2026-10-17).
- Surface: `Symbol.species`.
- Authority: the maintainer's decision of 2026-10-16.
- Required behavior: methods of `Array`, `%TypedArray%`, `Promise`, `RegExp`,
  `ArrayBuffer` and `SharedArrayBuffer` that the specification routes through
  `ArraySpeciesCreate`, `SpeciesConstructor` or `TypedArraySpeciesCreate`
  construct the intrinsic constructor without reading `constructor` or
  `@@species`. `Symbol.species` and the `get [Symbol.species]` accessors exist;
  nothing reads them. A subclass instance's `map`, `filter`, `slice`, `then` and
  the like return a base-class instance.
- Baseline evidence: [`dropped-variants.tsv`](../dev-support/test262/dropped-variants.tsv)
  lists 528 variants of reason `species`.
- Acceptance: `class A extends Array {}; new A(1, 2).map(x => x) instanceof A`
  is `false`. The variants of reason `species` fail by decision and are excluded
  from the Test262 metric's difference list; any other variant that fails is a
  defect.
- Specification:
  [SpeciesConstructor](https://tc39.es/ecma262/multipage/abstract-operations.html#sec-speciesconstructor),
  [ArraySpeciesCreate](https://tc39.es/ecma262/multipage/indexed-collections.html#sec-arrayspeciescreate).

Minimal probe:

```js
class A extends Array {} print(new A(1, 2).map(x => x) instanceof A);
```

### ECMASCRIPT-WEAKREF-LIFETIME-001

- Status: approved target; kept-object execution scopes and ordinary argument
  ownership transfer are implemented.
- Surface: WeakRef target liveness, FinalizationRegistry cleanup scheduling, and
  observable argument lifetimes during function calls.
- Authority: the maintainer explicitly directed that ECMAScript govern this
  behavior. Pinned QuickJS is a comparison implementation for its weak-reference
  results and argv-copy conditions.
- Required behavior: WeakRef construction and successful dereferencing keep
  targets alive through the current synchronous execution sequence via the
  effect of `AddToKeptObjects`. The host schedules `ClearKeptObjects` at an
  appropriate completed-execution boundary. Nested calls, parameter writes,
  and explicit GC must preserve this guarantee. FinalizationRegistry cleanup
  callbacks must not interrupt synchronous ECMAScript execution.
- Call ownership: preserve observable arguments values, mapped bindings,
  closure/eval access, rest values and suspended state. Ordinary calls may
  transfer argument ownership without a second original-argument owner.
  Neither universal retention until return nor immediate collection of dead
  values is a language requirement.
- Baseline evidence: pinned QuickJS evaluates
  `new WeakRef({}).deref() === undefined` to `true`. Rust returns the required
  `false`, with coverage for object and symbol targets, explicit GC, nested
  execution scopes, errors, panic cleanup and job dispatch.
  `Runtime::with_execution_turn` groups multiple embedding entries when the
  host requires one synchronous execution sequence. Job dispatch is rejected
  during an active turn and leaves the pending FIFO entry queued.
- Acceptance: cover construction and repeated successful dereferencing within
  one synchronous execution, nested host re-entry and GC, permitted clearing
  boundaries, asynchronous cleanup, and arguments/closure aliasing. Across a
  permitted clearing boundary, collection is permitted but optional.
- Specification: [WeakRef](https://tc39.es/ecma262/multipage/managing-memory.html#sec-weak-ref-objects),
  [liveness and execution](https://tc39.es/ecma262/multipage/executable-code-and-execution-contexts.html#sec-liveness),
  [ClearKeptObjects](https://tc39.es/ecma262/multipage/executable-code-and-execution-contexts.html#sec-clear-kept-objects).

### TEST262-ARROW-FOR-HEAD-001

- Status: approved target deviation.
- Approved by: review under the repository maintainer's standing delegation of
  Feature Parity implementation judgment.
- Surface: early-error parsing of an arrow expression in a classic `for`
  statement head created through the `Function` constructor.
- Exact Test262 keys:
  - `test/staging/sm/statements/arrow-function-in-for-statement-head.js` in the
    `sloppy` variant;
  - the same path in the `strict` variant.
- Upstream anchor: the pinned release's `test262_errors.txt` records both
  variants at `arrow-function-in-for-statement-head.js:13` because no
  `SyntaxError` is thrown.
- Rationale: retain the Rust parser's Test262-conforming early `SyntaxError`
  instead of reproducing this pinned QuickJS known failure. The constructed
  function body is parsed independently, so the same target difference is
  observable from both outer Test262 variants.
- Compatibility impact: source relying on pinned QuickJS accepting
  `for (x => 0 in 1;;) break;` through `Function` is rejected by Rust. The
  deviation does not broaden which arrow or `for` forms are rejected beyond
  this invalid grammar family.

Minimal probe:

```js
try {
  Function("for (x => 0 in 1;;) break;");
  print("accepted");
} catch (error) {
  print(error.name);
}
```

From the repository root:

```sh
./target/oracle/quickjs-2026-06-04/qjs -e 'try{Function("for (x => 0 in 1;;) break;");print("accepted")}catch(e){print(e.name)}'
cargo run --quiet --locked --bin qjs -- -e 'try{Function("for (x => 0 in 1;;) break;");print("accepted")}catch(e){print(e.name)}'
```

Pinned QuickJS prints `accepted`; Rust prints `SyntaxError`.

## QuickJS specification departures

Where pinned QuickJS departs from a clause of ECMA-262, quickjs-oxide follows
the clause. Each such behavior is registered in
[`quickjs-spec-departures.tsv`](../dev-support/quickjs-spec-departures.tsv)
with an identifier `QUICKJS-SPEC-NNN`, the clause, a summary and, where one
exists, the Test262 test that reaches it. The 44 entries cover, among others,
BigInt decrement outside `int64`, a bound function's prototype and
`toString`, the order of `Object.defineProperties`, the Proxy invariants and
traps, the Promise combinators, the Atomics, `ArrayBuffer`, `DataView` and
typed-array check orders, spread and `for await` iterator handling, the
iterator helpers, the async generator driver, for-in over keys added during
the loop, global declaration instantiation, `Math.sumPrecise`,
`BigInt.asIntN`/`asUintN`, and the RegExp and String protocols.

Every oracle case or fixture whose QuickJS output differs for one of these
reasons is attributed to its entry, in
[`oracle-attributions.tsv`](../dev-support/oracle-attributions.tsv) or the
`deviation` column of
[`quickjs-fixture-gates.tsv`](../dev-support/quickjs-fixture-gates.tsv), and
the oracle runner requires such a case to still differ from QuickJS.
`check-oracle-registry.py` requires every registry entry to be attributed and
every attribution to name a registry entry or a `TARGET-*` ledger entry.

What the specification leaves open follows QuickJS and is not a departure:
error message text and `stack`, the printer, the creation order of the
intrinsics' properties, the sort comparator's call sequence, for-in order
within its constraints, NaN bit patterns and resource limits (argument
counts, string length, nesting and recursion depths).

## Open specification departures

quickjs-oxide still matches pinned QuickJS in these areas where QuickJS
departs from the specification. Each is a defect to be returned to its
clause.

| Area | Behavior shared with QuickJS |
| --- | --- |
| scoping | per-iteration `for` bindings, `catch` and parameter scopes, binding patterns |
| Annex B | block-level functions; a global `var` beside a masked global lexical |
| syntax | early errors and accepted forms around `async` methods, class bodies and directives |
| TDZ | the error kind a function-body lexical write throws |
| early errors | a top-level lexical beside a function of the same name (16.1.1); `delete a?.#x` |
| templates | a template calls the patchable `String.prototype.concat` and converts after every substitution is evaluated (13.2.8.6) |
| references | `(a?.#m)()` loses its receiver; `null[k] = v` converts `k` before `ToObject`; a strict write to a sloppy function's name binding |
| lexer | `08.1_2`; `for (async <!-- …` |
| `%ThrowTypeError%` | it returns `undefined` when its receiver is a sloppy ordinary function, so `(function () {}).caller` is `undefined` (10.2.4.1, 17.1) |
| `Function.prototype.toString` of a built-in | prints the function's current own `name` instead of its `[[InitialName]]` (20.2.3.5) |

## Open implementation frontiers

- Deeply nested source throws a catchable `SyntaxError: stack overflow`, and
  the runtime continues afterward. QuickJS stops when its parser exceeds a
  one-MiB C stack budget, so its limit depends on C frame sizes and shifts by
  a few percent with the compiler that builds it. The Rust parser charges each
  recursive grammar edge a weight calibrated to one QuickJS reference build
  (`src/engine/ast/stack_guard.rs`), and a token-edge stack check stops
  productions that carry no weight. Single-production chains such as
  parentheses, arrays, blocks or arrows therefore throw within that margin of
  QuickJS. Chains that interleave several productions (a function body inside
  a call argument, a method body inside an object literal, class heritage,
  class element and default-parameter heads) are the sum of their edge
  weights and can throw at a different depth than QuickJS. QuickJS also names
  the production where its check trips, for example
  `missing formal parameter`, where Rust reports `stack overflow`. The `qjs`
  CLI evaluates on a 32 MiB (release) thread so that release builds reach
  these depths.
- `JSON.parse`, its reviver walk and JSON modules throw a catchable
  `stack overflow` past fixed nesting budgets: 10,891 containers around a leaf
  for parsing (`SyntaxError`) and 3,630 for the reviver (`InternalError`).
  QuickJS checks its one-MiB C stack instead, so its limits shift with the
  compiler that builds it, fall as the caller's stack deepens, and differ
  between reviver kinds and module import paths. The budgets match a top-level
  call on the reference build; under 500 JavaScript frames, QuickJS stops near
  6,850 and 2,280. Far beyond the budget, QuickJS reports the overflow one
  token earlier. Parsing also samples the host stack, so a thread too small
  for the budget throws earlier rather than overflowing.
