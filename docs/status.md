# Implementation status

quickjs-oxide is a Rust JavaScript engine available through its command-line application, embedding API, native adapter and browser playground. The crate layout and the host boundary are in the [architecture guide](architecture.md); [Execution](performance/execution.md) and [the heap](heap.md) describe the core's loop, caches, heap and collector, and [Execution and performance](performance/README.md) their measurement.

## Engine

- The front end (`src/engine/compiler`) parses Script and ECMAScript Module source, resolves bindings and emits stack code; the register lowering (`src/engine/compiler/register`) turns each function into a code unit of `quickjs-oxide-core`. The core validates each unit once when it is published.
- The core runs register bytecode in one loop over register windows, with inline caches for property reads and writes, global accesses and construction, a fast path for iterating ordinary arrays, and suspension for generators and async functions. `qjs --disassemble FILE` prints the register code.
- The heap holds NaN-boxed values, typed arenas, a value heap, shapes with validity cells and a mark-sweep collector that runs on heap growth. `RuntimeOptions` sets the collection threshold, its growth factor, a heap cap and the register file's limit; `MemoryReport` reports what every part of the runtime holds.
- The core implements objects, strings, numbers, BigInts, regular expressions (through `quickjs-oxide-regexp`), binary data, collections, weak references, Promises, jobs, modules, `SharedArrayBuffer`/`Atomics` and the corresponding builtins.
- A runtime has one realm. `with` is an early `SyntaxError`, direct `eval` runs as indirect eval, and nothing reads `Symbol.species` or `Symbol.unscopables`; the [deviation ledger](deviations.md) records each.
- The embedding API (`quickjs_oxide::engine::api::core`) exposes `Runtime`, `Context`, `Value`, the `Host` trait, host functions that receive a `Scope`, host objects, module loaders that receive a `ModuleScope`, `RuntimeOptions` and the collector, memory and inline-cache reports.
- Host adapters supply environment capabilities: `SystemHost` (`adapters/native`) and `WebHost` (`adapters/web`). Applications own file loading, output, diagnostics and job driving.
- `qjs` and the browser playground run the engine through the embedding API. Exact source bytes, malformed string data and diagnostic positions travel through the embedding and loader APIs.

## Test262 baseline

<!-- current-test262-metrics:start -->
The authoritative Test262 vector has:

- 81,441 passes out of the 83,562 variants in QuickJS 2026-06-04's scope (97.462%)
- 81,441 full-corpus passes out of 102,037 variants (79.815%)
- 83,354 eligible variants out of 102,037 (81.690%)
- 1,913 failures and no timeouts among the 83,354 eligible variants
<!-- current-test262-metrics:end -->

The profile, admission vector and result hashes are recorded in [`dev-support/test262/current.conf`](../dev-support/test262/current.conf). [Test262 guidance](test262.md) explains the runner and metric definitions.

<a id="verification"></a>
## Verification

```sh
cargo test --locked --workspace --all-targets
cargo test --locked --workspace --features test262-host --lib --bins
cargo test --locked -p quickjs-oxide-core --features gc-stress
cargo build -p quickjs-oxide-cli --features gc-stress --bin qjs
python3 scripts/checks/check-gc-stress.py --qjs target/debug/qjs
./scripts/checks/check-oracle-registry.sh
./scripts/quickjs/test-quickjs-fixtures.sh --all
./scripts/quickjs/test-quickjs-c-oracles.sh --check
./scripts/checks/test-parity-slice.sh
./scripts/test262/test-test262.sh --check
./scripts/test262/test-test262.sh --focused
TEST262_WORKERS=2 ./scripts/test262/test-test262.sh --full
./scripts/web/test-web-playground.sh
```

`check-gc-stress.py` runs the fixed probes and the core's corpus programs on `qjs --gc-stress=1`, collecting before every allocation, and compares their output with the normal run.
