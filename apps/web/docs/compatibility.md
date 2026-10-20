# JavaScript support

quickjs-oxide implements ECMAScript within the feature set of QuickJS 2026-06-04. ECMA-262 and Test262 decide what is correct. Where the specification leaves a behavior to the implementation, quickjs-oxide does what QuickJS 2026-06-04 does: error message text, `stack`, the value printer, the creation order of the builtins' properties, the call sequence of a sort comparator, for-in order within the specification's rules, NaN bit patterns and resource limits.

## Test262 results

Test262 runs at the commit the QuickJS 2026-06-04 release pins, with the release's `test262.patch` applied. QuickJS's `test262.conf` selects the scope. Each test runs once per variant (strict, sloppy, module), so the counts below are variants.

<!-- current-test262-metrics:start -->
| Measure | Passes | Variants | Rate |
| --- | ---: | ---: | ---: |
| In QuickJS 2026-06-04's scope | 81,441 | 83,562 | 97.462% |
| Whole Test262 corpus | 81,441 | 102,037 | 79.815% |
<!-- current-test262-metrics:end -->

The whole corpus has 18,475 more variants than QuickJS's scope. They are the ones QuickJS's own configuration skips: ECMA-402 `Intl`, Temporal, ShadowRealm, decorators, explicit resource management, proper tail calls, `Atomics.waitAsync`, `Array.fromAsync` and other proposals outside the release.

Within the scope, 1,913 executed variants fail:

- 1,870 fail by design. They exercise the five features listed under [Differences from QuickJS](#differences-from-quickjs).
- 43 are variants that QuickJS 2026-06-04 also fails, as listed in the release's `test262_errors.txt`.

The remaining 208 variants in the scope are outside the runner's current profile: Module tests whose import graphs the profile does not admit, and negative tests outside the audit of expected errors.

Every admitted negative test checks the error's type, message, line and column against QuickJS 2026-06-04, and a scheduled CI job replays each one against the pinned QuickJS.

[`docs/test262.md`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/docs/test262.md) defines each metric and gives the commands that reproduce them.

## Language and builtins

| Area | Support |
| --- | --- |
| Syntax | The ECMAScript language of QuickJS 2026-06-04: classes with private and static elements, destructuring, spread, optional chaining, generators, async functions and async generators, `for await`, BigInt literals, regular-expression literals with the `v` flag |
| Objects and functions | `Object`, `Function`, `Proxy`, `Reflect`, `Symbol`, `Error` and its subclasses, `WeakRef`, `FinalizationRegistry` |
| Numbers and text | `Number`, `Math` (including `Math.sumPrecise`), `BigInt`, `String`, `RegExp` with Unicode sets and `RegExp.escape`, `JSON`, `Date` |
| Collections | `Array` (including `toSorted` and the other copying methods), `Map`, `Set`, `WeakMap`, `WeakSet`, iterator helpers on `Iterator` |
| Binary data | `ArrayBuffer`, typed arrays including `Float16Array`, `DataView`, `SharedArrayBuffer`, `Atomics` |
| Control | `Promise` (including `Promise.withResolvers`), jobs, generators |
| Modules | Static and dynamic `import`, top-level `await`, import attributes, JSON modules, `import.meta` |

Strings are sequences of UTF-16 code units, and unpaired surrogates are preserved. The Unicode tables are Unicode 17.0.0.

## Programs and APIs

| Surface | Contents |
| --- | --- |
| `qjs` command line | `-e`, `-m`/`--module`, `--script`, `-s`, `--strip-source`, `--no-unhandled-rejection`, `--print-result`, `-q`; `print`, `console.log`, `scriptArgs`; `--std` defines `load` and `read` |
| `qjs` memory and diagnostics | `--heap-cap-mib`, `--min-heap-mib`, `--gc-growth`, `--stack-limit`, `--gc-stats`, `--memory-report`, `--disassemble`; `--ic-stats` in a build with the `ic-stats` feature |
| Rust embedding API | `Runtime`, `Context`, `Value`, `Host`, module loaders, `RuntimeOptions`, `MemoryReport` ([Embed quickjs-oxide](./embed.md)) |
| WebAssembly | The `quickjs-oxide-web` bindings ([WebAssembly](./webassembly.md)) |

The QuickJS `std` and `os` modules, the interactive REPL, `Worker`, `bjson`, compiled bytecode files, `qjsc` and the C API are not part of quickjs-oxide.

## Differences from QuickJS

These differences are deliberate. Each has an entry in the [deviation ledger](https://github.com/pocket-nexus/quickjs-oxide/blob/main/docs/deviations.md), with its exact scope and the Test262 variants it affects.

| Ledger entry | Behavior | Test262 variants |
| --- | --- | ---: |
| `TARGET-WITH-001` | A `with` statement is an early `SyntaxError` (`with statements are not supported`) in sloppy code, as in strict code | 311 |
| `TARGET-UNSCOPABLES-001` | `Symbol.unscopables` and `Array.prototype[Symbol.unscopables]` exist with their specified values; nothing reads them | 53 |
| `TARGET-DIRECT-EVAL-001` | A direct `eval(src)` call runs as indirect eval: in the global scope, without access to the caller's variables, `this`, `arguments` or `new.target` | 517 |
| `TARGET-REALMS-001` | A runtime has one realm; isolation uses separate runtimes, and `$262.createRealm` throws `TypeError: realms are not supported` | 488 |
| `TARGET-SPECIES-001` | Methods the specification routes through `Symbol.species` construct the builtin constructor; a subclass instance's `map`, `filter`, `slice` or `then` returns a base-class instance | 528 |

Of the 1,897 variants in this table, 27 pass anyway: negative `with` tests whose expected early error the front end reports either way.

Three of them, each a separate script:

```js
with ({ x: 1 }) {}
// SyntaxError: with statements are not supported
```

```js
var x = 1;
function f() { var x = 2; return eval("x"); }
f(); // 1
```

```js
class A extends Array {}
new A(1, 2).map((v) => v) instanceof A; // false
```

Two more ledger entries follow ECMAScript where QuickJS 2026-06-04 does not:

- `ECMASCRIPT-WEAKREF-LIFETIME-001`: a `WeakRef` keeps its target alive until the end of the current synchronous execution, so `new WeakRef({}).deref() === undefined` is `false`. `FinalizationRegistry` callbacks never interrupt synchronous code.
- `TEST262-ARROW-FOR-HEAD-001`: `Function("for (x => 0 in 1;;) break;")` throws `SyntaxError`, as the grammar requires.

### Where QuickJS departs from the specification

Where QuickJS 2026-06-04 departs from a clause of ECMA-262, quickjs-oxide follows the clause. The 44 such behaviors are registered as `QUICKJS-SPEC-NNN` in [`quickjs-spec-departures.tsv`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/dev-support/quickjs-spec-departures.tsv). They include BigInt decrement outside the 64-bit range, a bound function's prototype and `toString`, the order of `Object.defineProperties`, Proxy invariants and traps, the Promise combinators, check orders in `Atomics`, `ArrayBuffer`, `DataView` and typed arrays, iterator handling in spread and `for await`, the iterator helpers, the async generator driver, `Math.sumPrecise`, `BigInt.asIntN` and `BigInt.asUintN`, and the RegExp and String protocols.

In a few areas quickjs-oxide behaves as QuickJS does where both depart from the specification. The ledger lists them as defects:

| Area | Behavior shared with QuickJS |
| --- | --- |
| Scoping | Per-iteration `for` bindings, `catch` and parameter scopes, binding patterns |
| Annex B | Block-level functions; a global `var` beside a masked global lexical binding |
| Syntax | Early errors and accepted forms around `async` methods, class bodies and directives |
| TDZ | The error kind of a write to a function-body lexical binding before initialization |
| Templates | A template calls the patchable `String.prototype.concat` and converts after evaluating every substitution |
| References | `(a?.#m)()` loses its receiver; `null[k] = v` converts `k` before `ToObject` |
| `%ThrowTypeError%` | `(function () {}).caller` is `undefined` |
| `Function.prototype.toString` of a builtin | Prints the function's current own `name` |

### Nesting limits

Deeply nested source throws a catchable `SyntaxError: stack overflow`, and the runtime keeps working afterwards. The parser charges each nesting level a weight calibrated against QuickJS, so simple chains (parentheses, arrays, blocks) overflow within a few percent of QuickJS's depth; chains that mix several constructs can overflow at a different depth, and the message always says `stack overflow`. The `qjs` command line evaluates on a 32 MiB thread in release builds.

`JSON.parse` accepts 10,891 levels of nesting and its reviver 3,630. Deeper input throws a catchable `stack overflow` (`SyntaxError` from parsing, `InternalError` from the reviver).

## Where to go next

- [Performance](./performance.md): speed and memory against QuickJS 2026-06-04.
- [Errors and limits](./limits.md): exceptions, engine errors and resource limits.
- [Architecture](./architecture.md): how a script is compiled and run.
- [`docs/compatibility.md`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/docs/compatibility.md): the compatibility baseline and differential testing.
