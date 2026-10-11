# Deviation ledger

This ledger lists the approved differences from QuickJS 2026-06-04 described in
[compatibility](compatibility.md). Each approved target deviation covers exactly
the observable behavior and test variants named below; any other mismatch is a
defect.

## Approved target deviations

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

### TEST262-ANNEXB-EVAL-001

- Status: approved target deviation.
- Approved by: review under the repository maintainer's standing delegation of
  Feature Parity implementation judgment.
- Surface: Annex B eval function declarations inside a `with` environment.
- Exact Test262 key:
  `test/staging/sm/lexical-environment/block-scoped-functions-annex-b-eval.js`
  in the `sloppy` variant (`noStrict`).
- Upstream anchor: the pinned release's `test262_errors.txt` records the
  location `block-scoped-functions-annex-b-eval.js:11`. Pinned QuickJS produces
  `outer-gouter-geval-gtruefalseq`; the assertion expects
  `outer-geval-gwith-gtruefalseq`.
- Rationale: retain the Rust engine's Test262-conforming Annex B.3.3.3 result
  instead of reproducing this pinned QuickJS known failure. The declaration
  updates the eval variable environment without replacing the `with` object's
  own `g` property.
- Compatibility impact: code depending on the pinned bug observes the outer
  binding change and the `with` property remain unchanged in Rust. The
  deviation is limited to the exact binding interaction represented by the
  Test262 key above.

Minimal probe:

```js
var log = "";
function f() {
  log += g();
  function g() { return "outer-g"; }
  var o = { g: function () { return "with-g"; } };
  with (o) {
    eval('{ function g() { return "eval-g"; } }');
  }
  log += g();
  log += o.g();
}
f();
print(log);
```

From the repository root, run the probe as one line with the pinned oracle and
the Rust CLI:

```sh
./target/oracle/quickjs-2026-06-04/qjs -e 'var log="";function f(){log+=g();function g(){return "outer-g"}var o={g:function(){return "with-g"}};with(o){eval("{ function g(){ return \"eval-g\"; } }")}log+=g();log+=o.g()}f();print(log)'
cargo run --quiet --locked --bin qjs -- -e 'var log="";function f(){log+=g();function g(){return "outer-g"}var o={g:function(){return "with-g"}};with(o){eval("{ function g(){ return \"eval-g\"; } }")}log+=g();log+=o.g()}f();print(log)'
```

Pinned QuickJS prints `outer-gouter-geval-g`; Rust prints
`outer-geval-gwith-g`.

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

## Open implementation frontiers

- Deeply nested source throws a catchable `SyntaxError: stack overflow`, and
  the runtime continues afterward. QuickJS stops when its parser exceeds a
  one-MiB C stack budget, so its limit depends on C frame sizes and shifts by
  a few percent with the compiler that builds it. The Rust parser charges each
  recursive grammar edge a weight calibrated to one QuickJS reference build
  (`src/engine/compiler/stack_guard.rs`), and a token-edge stack check stops
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
- Dynamic import retries failed acyclic source graphs as pinned QuickJS does.
  Parse-in-progress definitions and request prefixes are published in the same
  callback order as pinned QuickJS, including its one-shot resolution latch.
  Rust represents a swallowed resolution failure as an explicit incomplete
  state with no partial raw dependency vector; direct link/execute returns a
  typed error and dynamic import rejects with a deterministic `InternalError`.
  If a resolved module still refers to a construction that later fails, Rust
  keeps that append-only identity as an edge-free `Aborted` sentinel, excludes
  it from name lookup, and gives a same-name retry a new identity. Pinned
  QuickJS can instead retain a dangling `JSModuleDef *`; subsequent use enters
  native undefined behavior and may manifest as a native crash or allocator
  aliasing. The automated oracle suite omits those unsafe operations, and
  reproducing those lifetimes is outside the parity target. Rust keeps vacant
  module-cache slots and orphaned sentinels uncompacted, so repeated failed
  publications are a resource-hygiene frontier.
- Admitted ordinary async-generator function, object-method, and public
  class-method direct-yield/await paths match the pinned driver, including
  poisoned Promise constructors, iterator-result resolution reentry, and
  completed-state queue re-entry. Internal allocation/setup failure recovery
  differs: iterator-result allocation can fail after the VM has advanced, and
  failure to create or install a private continuation can leave the front
  request capability pending for a later retry. Admitting these host-error
  paths requires fault injection and a transactional pending-settlement
  representation; they are an open hardening frontier.
- `Promise.all`, `Promise.allSettled`, and `Promise.any` match pinned QuickJS on
  ordinary JavaScript-observable paths; internal allocation failure routes
  differently. Failure to allocate the output Array returns a host runtime
  error instead of rejecting the new capability, and failure to allocate an
  element callback skips QuickJS's close-then-reject path. Internal `AggregateError` allocation has the same
  runtime-error boundary. The checked `u32` element index has a theoretical
  multi-billion-element RangeError boundary instead of QuickJS's C `int`
  environment behavior. These are open hardening frontiers.
