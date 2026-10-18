# Compatibility

quickjs-oxide targets the observable behavior of **QuickJS 2026-06-04**: the
same input produces the same results, errors and side effects as that release.
The engine is written entirely in Rust with `unsafe_code = "forbid"`. Product
builds compile, link and run only Rust code; the pinned QuickJS serves as the
test oracle.

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

Each failing variant is one that QuickJS 2026-06-04 also fails with the same
error, as listed in the release's `test262_errors.txt`. The variants not yet
run are Module tests whose import graphs are not yet admitted and negative
tests outside the diagnostic audit. The [Test262 guide](test262.md) defines
the metrics and how to reproduce them.

Early errors follow QuickJS's diagnostics. Every admitted negative test has an
exact contract with QuickJS's error type, message, line and column, and
scheduled CI replays each contract against the pinned QuickJS.

## Differential testing

The oracle suite in `apps/cli/tests/oracle` runs each case through the pinned
QuickJS and the Rust engine and compares results, exceptions and side
effects. Fixtures under `apps/cli/tests/fixtures` freeze QuickJS outputs for
the CLI and module host; `scripts/quickjs` rebuilds the pinned QuickJS to
regenerate and verify them.

## Intentional differences

The [deviation ledger](deviations.md) lists every approved difference from
QuickJS 2026-06-04, each with its exact scope and Test262 variants:

- WeakRef targets and call arguments follow ECMAScript lifetimes.
- Annex B function declarations from `eval` inside `with` follow ECMAScript.
- An arrow function in a classic `for` head is parsed as ECMAScript requires.

Any other observable difference is a defect.

## Product surface

| Area | Status |
| --- | --- |
| Language, built-ins, Promise jobs, `WeakRef`/`FinalizationRegistry`, `SharedArrayBuffer`/`Atomics` | Supported |
| ECMAScript modules: static and dynamic `import`, top-level `await`, import attributes, JSON modules, `import.meta` | Supported |
| `qjs` CLI: `-e`, `-m`/`--script`, `-s`, `--strip-source`, `--no-unhandled-rejection`, `-q`, `-d`, `-T`; `print`, `console.log`, `scriptArgs` | Supported |
| Rust embedding API: `Runtime`, `Context`, host services and module loader callbacks | Supported |
| WebAssembly bindings and the [browser playground](playground.md) | Supported |
| `std` and `os` modules (`--std`): files, processes, timers and the event loop | Not yet supported |
| Interactive REPL (`-i`) | Not yet supported |
| CLI options `--strict`, `-I`/`--include`, `--memory-limit`, `--stack-size` | Not yet supported |
| `Worker` | Not yet supported |
| `bjson` object serialization and compiled bytecode files | Not yet supported |
| `qjsc` compiler | Not yet supported |
| C API, shared library and native C modules | Not yet supported |
