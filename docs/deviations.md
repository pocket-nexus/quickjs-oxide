# Deviation ledger

This ledger is subordinate to [`parity.md`](parity.md). Approved target
deviations are limited to the exact observable behavior and test variants named
below; they do not authorize adjacent differences. An unsupported feature or an
unresolved mismatch still blocks the relevant parity claim and is not silently
accepted as a deviation.

## Approved target deviations

### TEST262-ANNEXB-EVAL-001

- Status: approved target deviation on 2026-08-09.
- Approved by: the architecture-hygiene milestone review, under the repository
  maintainer's standing delegation of Feature Parity implementation judgment.
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

- Status: approved target deviation on 2026-08-09.
- Approved by: the architecture-hygiene milestone review, under the repository
  maintainer's standing delegation of Feature Parity implementation judgment.
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

### PARSER-DEPTH-COMPOUND-FAMILY-002

- Status: registered target deviation on 2026-09-19, as part of task B8-r4
  (review 1089 B-C follow-up). This is a narrowly-scoped residual around the
  already-approved PARSER-DEPTH-WITH-OBJECT-HEAD-001 (now resolved); the
  standalone families, the direct `-e` root, and the five uncaught-column
  families are byte-exact.
- Approved by: the maintainer's standing delegation of robustness-gate
  implementation judgment for B8; the cross-task reviewer reconciles the
  exact-depth residuals here.
- Surface: the exact nesting depth at which the parser stack-overflow early
  error fires for *compound/transient* productions whose pinned C frame
  residency the weighted Rust model does not reproduce one-to-one. In every
  case the error type, message, catchability and process exit code match; only
  the first-throw depth (and, for a few forms, the specific early diagnostic)
  differs.
- Upstream anchor: pinned `next_token()` (`quickjs.c:22719`) bounds parser
  recursion with one physical 1 MiB byte budget; compound productions fail at
  different depths purely because their transient C frames differ in size.
  The Rust weighted budget models each grammar edge with one calibrated weight
  for the standalone forms. These residuals are shapes whose deepest nesting
  stacks two or three distinct transient frames (a function body inside a call
  argument, a getter/method body inside an object literal, a class heritage
  parenthesis, a class field/method initializer, or a default-parameter
  function), and the exact mix of simultaneously-resident C frames is not a
  linear combination of the standalone weights.

Three distinct, measurable context shifts remain:

1. **Eval compound families** (top-level `eval(...)`). First-throw depth
   (oxide → pinned), all `SyntaxError: stack overflow` unless noted:

   | form | oxide | pinned |
   |---|---:|---:|
   | `f(function(){…})` (fn-expr-in-call) | 578 | 295 |
   | `({get a(){…}})` / `({m(){…}})` | 313 | 284 |
   | `({[…]:0})` (obj-computed-key) | 355 | 344 |
   | `class C extends (…){}` | 718 | 537 |
   | `class C{[…]=0}` (class-field-computed) | 3289 | 574 |
   | `class C{m(){…}}` (class-method-body) | 2613 | 1021 |
   | `class C{static{…}}` | 8837 (`SyntaxError`) | 719 (`InternalError`) |
   | `function f(a=…){}` (param-default-fn) | 3352 (`stack overflow`) | 642 (`missing formal parameter`) |

   In all rows oxide accepts deeper than pinned (conservative), except none
   accept shallower. The `class C{static{` form differs additionally in error
   *name* (`SyntaxError` vs pinned's VM-side `InternalError`) and the
   default-parameter form in the specific message (`stack overflow` vs
   `missing formal parameter`); both still throw a catchable early error.

2. **File/module roots for the light conditional/assignment frames.** A script
   loaded from a file (`qjs file.js`) or a module root runs one native C frame
   deeper than a `qjs -e` command line in pinned QuickJS (`eval_file` wraps
   `eval_buf`), so conditional/cond-alternate/assign/compound-assign first
   throw at **8174** for a file/module but **8175** for `-e`. quickjs-oxide
   uses one Direct context for both and first throws at 8175 in each (one level
   permissive for file/module input). Every heavier file/module family matches.

3. **`Function(...)` constructor bodies.** Pinned assembles and compiles the
   body via indirect eval from `js_eval_function`, starting at a different
   native depth and inside a wrapper function. Of the 63 forms, 8 shallow
   grammar probes agree and the three spread/pattern runtime-message probes
   are a separate pre-existing `Symbol.iterator` difference; the remaining
   forms first throw slightly later in oxide. Representative first-throw rows
   (oxide → pinned): `paren` 716→715, `array`/`call`
   741→739, `block` 3261→3252, `if`/`switch`/`label` 3431–3432→3423,
   `conditional`/`assign` 8149→8128, `unary` 9313→9289,
   `class-extends` 716→534, `class-static-block` 8831→1016,
   `class-method-body` 2608→1016, `param-default-fn` 3350→639,
   `fn-expr-in-call` 577→293.

4. **Nested-block statement uncaught columns (depth still exact).** For the
   headed loops whose controlled body is itself a block, the uncaught
   diagnostic column stays one head-width early even though the first-throw
   *depth* matches: `while(0){…}` / `do{…}while` / `with(1){…}` /
   `with({}){…}` report the column one early (e.g. nested `while`
   `<cmdline>:1:15093` vs pinned `15094`), and `for(;;){…}` seven early
   (`23472` vs `23479`). Block, if, label, new and import — the five families
   named in review 1089 B-D — are byte-exact (block 3271, if 17211, label
   19543, new 23761, import 5215), as are their depths. Closing the residual
   would require splitting the shared statement-head/block weights by
   controlled-body shape, which would perturb the 30+ already-exact families;
   the error message, name, depth and exit code match and only the uncaught
   column differs.

5. **Spread chains with a non-array leaf (one-level conservative, pre-existing;
   confirmed unchanged by B8-r5).** Nested `[...` chains whose innermost leaf
   is not an array literal (`[...[...1]]`, leaf `1`) first throw one level
   later in oxide than pinned: **eval 744 vs 743**, **direct root 745 vs
   744** (the bracketed-leaf form `[...[...[]]]` is exact at eval 743 / direct
   744). The weighted model ties one charge to entering the innermost array
   primary; pinned's `next_token` check fires on the same token-edge count
   regardless of the leaf shape, so the non-array leaf leaves one frame of
   headroom in the model. The shift is conservative (oxide accepts one more
   nesting level, then throws the identical catchable
   `SyntaxError: stack overflow`; name/message/exit code match), predates
   B8-r4 and is byte-identical on `2372d2c2` and the B8-r5 branch; no shallow
   program is affected.

- Rationale: the weighted budget plus physical backstop already guarantee the
  parity.md:77 contract — a catchable `SyntaxError: stack overflow`, never a
  process abort — for every production and entry point, and all *standalone*
  families plus the direct `-e` root and the five B-D column families are
  byte-exact at the pinned depths. Removing the residual depth deltas would
  require per-compound frame tables (or a third `Function`-ctor weight context
  and a file-versus-command-line origin flag) modeling transient C frame
  residency that the Rust recursive-descent parser does not share, for inputs
  no realistic program reaches (hundreds to thousands of nested getter/class/
  default-parameter heads). The conservative direction (oxide accepts deeper,
  then throws the same catchable error) cannot reject source pinned accepts.
- Compatibility impact: only programs nesting the listed compound shapes to
  these extreme depths observe a different first-throw depth; the thrown value
  is catchable and has the same name/message for the standalone-shape rows, the
  runtime continues afterward, and an uncaught throw still exits 1. No shallow
  program (including all of test262) is affected. The tables above retain the measured residuals from the 63-family ×
  five-entry review matrix; minimal reproduction commands follow below.
  - 2026-09-19 B8-r4 interim regression, closed by B8-r5 (task 1178): while
    charging the immediate spread-array operand, the operand was parsed by a
    direct `parse_array_literal` call instead of the full AssignmentExpression,
    so shallow continuations pinned accepts (`[...[1].map(x=>x)]`,
    `[...[1] || []]`, `[...[1] ? [2] : [3]]`) were rejected with
    `expecting ']'`. That contradicted an earlier draft of this bullet; it was
    not an accepted deviation and is now fixed. The spread operand again goes
    through `parse_assignment_allow_in` (mirroring `js_parse_assign_expr`,
    quickjs.c:25732-25738), and only the leading `[` primary swaps in the
    smaller `SpreadElement` charge via a one-shot parser flag, preserving the
    pinned spread-array boundaries (eval 743, direct 744). Regression coverage
    lives in `tests/spread_operand.rs` (byte-compared with pinned under
    `QJS_ORACLE`).

Minimal probes:

```sh
# file-input light-frame shift (file: pinned throws at 8174, oxide at 8175)
python3 -c "print('1?'*8174+'1'+':0'*8174)" > /tmp/cond.js
qjs /tmp/cond.js;            cargo run --locked --bin qjs -- /tmp/cond.js
# eval compound family (getter body)
qjs -e 'try{eval("({get a(){return ".repeat(284)+"0"+"}})".repeat(284));print("ok")}catch(e){print(e.name)}'
```

## Resolved findings

### PARSER-DEPTH-WITH-OBJECT-HEAD-001

- Status: resolved on 2026-09-19 by task B8-r4; exact depth parity restored.
- Original surface: nested `with` whose head discriminant was an object
  literal, `eval("with({}){".repeat(n) + "0" + "}".repeat(n))`, first threw at
  depth 1674 in quickjs-oxide versus 1675 in pinned QuickJS.
- Resolution: the with-head object literal now carries a dedicated transient
  frame charge, and the statement-head weight is charged only after the head is
  consumed (at the controlled-body token). The eval form now accepts through
  1674 and first throws at 1675, and a direct Script/Module root accepts
  through 1676 and first throws at 1677, byte-for-byte matching pinned
  QuickJS including the uncaught diagnostic location. Regression coverage:
  `tests/parser_stack_depth.rs` (eval and direct boundaries).

### FORIN-FAST-ARRAY-001

- Status: resolved on 2026-07-15; no deviation approval requested.
- Surface: representation-sensitive Array mutation during `for-in`.
- Upstream anchor: `quickjs.c` 16282-16509.
- Compatibility impact while open: a deleted dense own index could incorrectly
  hide an inherited key, and a newly added own key could fail to hide one.

Minimal deletion probe:

```js
(function () {
  var p = [];
  p[1] = "proto";
  var a = [0, 1], out = "";
  Object.setPrototypeOf(a, p);
  for (var key in a) {
    out += key + ",";
    if (key === "0") delete a[1];
  }
  return out;
})()
```

Pinned QuickJS and the current Rust engine both return `0,1,`. A second
differential forces the same current key set through `Object.defineProperty` or
sparse growth first; both engines then retain the slow representation and
return `0,`.

The fix records QuickJS's irreversible fast/slow Array state in the heap
payload. A count-only fast iterator refreshes the source's current own names
when prototype enumeration becomes necessary; a slow iterator retains its
initial snapshot. Both paths are pinned in
`apps/cli/tests/oracle/control_flow/oracle_for_in.rs`.

## Open implementation frontiers

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
  aliasing. Those unsafe operations are deliberately excluded from the
  automated oracle suite. Reproducing those lifetimes is not an approved
  parity target. Vacant Rust module-cache slots and orphaned sentinels are
  currently not compacted, so repeated failed publications remain a
  resource-hygiene frontier.
- Admitted ordinary async-generator function, object-method, and public
  class-method direct-yield/await paths match the pinned driver, including
  poisoned Promise constructors, iterator-result resolution reentry, and
  completed-state queue re-entry.
  Internal allocation/setup
  failure is not yet recovered identically: iterator-result allocation can
  fail after the VM has advanced, and failure to create or install a private
  continuation can leave the front request capability pending for a later
  retry. These host-error paths require fault injection and a transactional
  pending-settlement representation before they can be admitted; they are an
  unresolved hardening frontier, not an approved observable deviation.
- `Promise.all`, `Promise.allSettled`, and `Promise.any` match pinned QuickJS on
  ordinary JavaScript-observable paths, but internal allocation failure is not
  yet routed identically. Failure to allocate the output Array currently
  returns a host runtime error instead of rejecting the new capability;
  failure to allocate an element callback also omits QuickJS's
  close-then-reject path. Internal `AggregateError` allocation has the same
  runtime-error boundary. The checked `u32` element index has a theoretical
  multi-billion-element RangeError boundary instead of QuickJS's C `int`
  environment behavior. These are unresolved hardening frontiers, not
  approved deviations.
