# Compatibility

quickjs-oxide targets **ECMA-262 within the feature set of QuickJS
2026-06-04**, apart from the five deviation-ledger entries below. ECMA-262 and
Test262 are the source of truth for correctness. QuickJS 2026-06-04 is the
reference where the specification leaves a behavior to the implementation or
the host: error message text, `stack`, the printer, the creation order of the
intrinsics' properties, the sort comparator's call sequence, for-in order
within its constraints, NaN bit patterns and resource limits. The engine is
written entirely in Rust with `unsafe_code = "forbid"`. Product builds
compile, link and run only Rust code; the pinned QuickJS serves as the test
oracle.

## Baseline

- QuickJS release 2026-06-04, pinned with its archive checksum in
  [`compat/upstream.toml`](../compat/upstream.toml): bytecode version 5 and
  Unicode 17.0.0.
- Test262 at the commit the release's `Makefile` pins, with the release's
  `tests/test262.patch` applied and its `test262.conf` selecting the scope.
- The target matches the release's feature set. Features its `test262.conf`
  skips (ECMA-402 `Intl`, Temporal, ShadowRealm, decorators, explicit resource
  management, proper tail calls, `Atomics.waitAsync`, `Array.fromAsync` and the
  other skipped proposals) lie outside the target, as they do for QuickJS.

## Language conformance

<!-- current-test262-metrics:start -->
- 81,441 of the 83,562 Test262 variants in QuickJS 2026-06-04's scope pass (97.462%).
- 1,913 executed variants fail, and 208 variants in that scope are not yet run.
<!-- current-test262-metrics:end -->

Of the 1,913 failing variants, 1,870 fail by decision: they are in the
dropped-variant list of the five `TARGET-*` ledger entries. The other 43 are
variants that QuickJS 2026-06-04 also fails, as listed in the release's
`test262_errors.txt`. The variants not yet run are Module tests whose import
graphs are not yet admitted and negative tests outside the diagnostic audit.
The [Test262 guide](test262.md) defines the metrics and how to reproduce
them.

Early errors follow QuickJS's diagnostics. Every admitted negative test has an
exact contract with QuickJS's error type, message, line and column, and
scheduled CI replays each contract against the pinned QuickJS.

## Differential testing

The oracle suite in `apps/cli/tests/oracle` runs each case through the pinned
QuickJS and `qjs` and compares results, exceptions and side effects. Fixtures
under `apps/cli/tests/fixtures` freeze QuickJS outputs for the CLI and module
host; `scripts/quickjs` rebuilds the pinned QuickJS to regenerate and verify
them. The oracle finds defects; it does not define correctness. A case that
differs from QuickJS because of a ledger entry or a registered QuickJS
specification departure is attributed to it in
[`oracle-attributions.tsv`](../dev-support/oracle-attributions.tsv) (fixtures:
the `deviation` column of
[`quickjs-fixture-gates.tsv`](../dev-support/quickjs-fixture-gates.tsv)), and
the runner requires it to still differ.

## Intentional differences

The [deviation ledger](deviations.md) lists every approved difference from
QuickJS 2026-06-04, each with its exact scope and Test262 variants:

- `with` is an early `SyntaxError` in sloppy code (`TARGET-WITH-001`).
- `Symbol.unscopables` exists; nothing reads it (`TARGET-UNSCOPABLES-001`).
- `eval(src)` at a direct-eval call site runs as indirect eval in the global
  scope (`TARGET-DIRECT-EVAL-001`).
- A runtime has one realm; `$262.createRealm` throws `TypeError: realms are
  not supported` (`TARGET-REALMS-001`).
- Methods that the specification routes through `Symbol.species` construct
  the intrinsic constructor; `Symbol.species` exists and nothing reads it
  (`TARGET-SPECIES-001`).
- WeakRef targets and call arguments follow ECMAScript lifetimes
  (`ECMASCRIPT-WEAKREF-LIFETIME-001`).
- An arrow function in a classic `for` head is parsed as ECMAScript requires
  (`TEST262-ARROW-FOR-HEAD-001`).

Where QuickJS departs from a clause of ECMA-262, quickjs-oxide follows the
clause; each such behavior is registered as `QUICKJS-SPEC-NNN` in
[`quickjs-spec-departures.tsv`](../dev-support/quickjs-spec-departures.tsv).
The ledger also lists the [areas](deviations.md#open-specification-departures)
where quickjs-oxide still shares a QuickJS departure; those are defects.

Any other observable difference from QuickJS where the specification leaves
the behavior open, and any difference from the specification, is a defect.

## Product surface

| Area | Status |
| --- | --- |
| Language, built-ins, Promise jobs, `WeakRef`/`FinalizationRegistry`, `SharedArrayBuffer`/`Atomics` | Supported |
| ECMAScript modules: static and dynamic `import`, top-level `await`, import attributes, JSON modules, `import.meta` | Supported |
| `qjs` CLI: `-e`, `-m`/`--module`, `--script`, `-s`, `--strip-source`, `--no-unhandled-rejection`, `--print-result`, `-q`; `print`, `console.log`, `scriptArgs`; `--std` defines `load` and `read` | Supported |
| `qjs` memory and diagnostics: `--heap-cap-mib`, `--min-heap-mib`, `--gc-growth`, `--stack-limit`, `--gc-stats`, `--memory-report`, `--disassemble`, `--ic-stats` (feature `ic-stats`) | Supported |
| Rust embedding API: `Runtime`, `Context`, `Host`, module loaders, `RuntimeOptions`, `MemoryReport` | Supported |
| WebAssembly bindings and the [browser playground](playground.md) | Supported |
| `std` and `os` modules: files, processes, timers and the event loop | Not yet supported |
| Interactive REPL (`-i`) | Not yet supported |
| CLI options `--strict`, `-I`/`--include`, `--memory-limit`, `--stack-size` | Not yet supported |
| `Worker` | Not yet supported |
| `bjson` object serialization and compiled bytecode files | Not yet supported |
| `qjsc` compiler | Not yet supported |
| C API, shared library and native C modules | Not yet supported |
