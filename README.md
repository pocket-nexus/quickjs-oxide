# quickjs-oxide

A JavaScript engine written entirely in safe Rust, with zero `unsafe`, that
targets the behavior of [QuickJS](https://bellard.org/quickjs/) 2026-06-04.
Its core, `quickjs-oxide-core`, compiles every function to register bytecode and runs
it in one loop with inline caches, over a NaN-boxed value heap and a
mark-sweep collector.

<!-- current-test262-metrics:start -->
Within the Test262 scope that QuickJS 2026-06-04 runs, quickjs-oxide passes
**81,441 of 83,562 variants (97.462%)**. QuickJS's `test262.conf`
skips the other 18,475 of the 102,037 variants; across the full corpus the
pass rate is 79.815%.
<!-- current-test262-metrics:end -->

- **Zero `unsafe`.** Every crate in the workspace sets
  `unsafe_code = "forbid"`, so the compiler rejects `unsafe` anywhere in the
  engine, the CLI and the WebAssembly bindings. The collector, the compiler,
  the loop, the regular expression engine and the Unicode tables are all safe
  Rust; the only third-party code in the engine is `num-bigint` and
  `num-traits`, for the front end's BigInt literals and the embedding API's
  BigInt values.
- **Performance close to QuickJS.** On the 37 fixed-work programs of the
  V8 v7, Octane 2.0 and AWFY suites, quickjs-oxide takes 0.94× QuickJS
  2026-06-04's time over V8 v7's eight programs, 1.15× over Octane's 15 and
  0.99× over AWFY's 14 (geometric means of release builds on one Linux
  machine; `scripts/benchmark/suite_matrix.py` runs the
  [matrix](scripts/benchmark/README.md#fixed-work-v8-v7-octane-and-awfy-matrix)).
- **ECMAScript first, QuickJS where it is open.** ECMA-262 and Test262 decide
  correctness; QuickJS 2026-06-04 decides what the specification leaves to
  the implementation (error messages, `stack`, the printer, property order of
  the intrinsics), and a differential suite compares the two engines case by
  case. Five behaviors depart by decision: `with` is a `SyntaxError`, nothing
  reads `Symbol.unscopables`, direct `eval` runs as indirect eval, a runtime
  has one realm, and nothing reads `Symbol.species`
  ([deviation ledger](docs/deviations.md)).
- **Runs where you need it.** A `qjs` command line, a Rust embedding API and a
  WebAssembly build.

**[Try it in the browser →](https://pocket-nexus.github.io/quickjs-oxide/)**
The playground runs the engine compiled to WebAssembly and reports the exact
build it runs.

## Install

[GitHub Releases](https://github.com/pocket-nexus/quickjs-oxide/releases)
carry prebuilt `qjs` binaries for Linux, macOS and Windows, WebAssembly
packages and a versioned playground.

```sh
cargo binstall --git https://github.com/pocket-nexus/quickjs-oxide quickjs-oxide-cli
# or build from source with Rust 1.99 or newer
cargo install --locked --git https://github.com/pocket-nexus/quickjs-oxide quickjs-oxide-cli
```

## Command line

```sh
qjs -e 'console.log({ answer: 42 })'
qjs --print-result -e '(function (a) { return a + 1; })(41)'  # 42
qjs module.mjs            # ES modules: -m/--module, --script, or .mjs detection
qjs data.js arg1 arg2     # arguments are available as scriptArgs
```

The file loader follows QuickJS: `.json` imports are strict JSON, and
`with { type: "json5" }` selects QuickJS's extended JSON.

## Embedding

```toml
[dependencies]
quickjs-oxide = { git = "https://github.com/pocket-nexus/quickjs-oxide" }
quickjs-oxide-host = { git = "https://github.com/pocket-nexus/quickjs-oxide" }
```

```rust
use quickjs_oxide::engine::api::core::{Runtime, Value};
use quickjs_oxide_host::SystemHost;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let mut context = runtime.new_context()?;
    let value = context.eval("[1, 2, 3].map((x) => x * 2).join()")?;
    if let Value::String(text) = value {
        println!("{}", text.to_utf8_lossy()); // 2,4,6
    }
    Ok(())
}
```

A Rust closure becomes a JavaScript function; its arguments are converted
to the closure's parameter types:

```rust
use quickjs_oxide::engine::api::core::{Runtime, Scope};

let runtime = Runtime::new();
let context = runtime.context();
let global = context.global_object()?;
context.define_function(&global, "add", 2, |_: &mut Scope<'_>, a: f64, b: f64| Ok(a + b))?;
assert_eq!(context.eval("add(40, 2)")?.as_number(), Some(42.0));
```

`examples/eval.rs` handles every completion value and uncaught exceptions;
`examples/host.rs` shows host functions, a host object, a promise settled
from Rust, a native module and an interrupt.
Code compiled through `eval`/`compile` lives as long as the runtime; an
application that evaluates input repeatedly (a REPL) passes
`EvalOptions::default().transient()` so that each input's code is freed
once unreachable ([code lifetime](src/engine/api/README.md#code-lifetime)).
A host (`SystemHost`, or an application's own `Host`) supplies time,
randomness and output; a module loader lets an application resolve and load
ES modules. A runtime has one realm; separate runtimes isolate code.

## Compatibility

The language, built-ins and ES modules (static and dynamic `import`,
top-level `await`, import attributes, JSON modules) follow ECMA-262 within
QuickJS 2026-06-04's feature set, apart from the five ledger entries above.
The `std` and `os` modules, the REPL, `Worker`, `qjsc` and the C API are not
yet supported; the Rust embedding API provides the C API's equivalents. [Compatibility](docs/compatibility.md) details the baseline,
how conformance is measured, the approved deviations and the full product
surface.

## Documentation

- [Compatibility with QuickJS 2026-06-04](docs/compatibility.md)
- [Architecture](docs/architecture.md) and [implementation status](docs/status.md)
- [Test262 baseline and metrics](docs/test262.md)
- [Execution](docs/performance/execution.md), [the heap](docs/heap.md), [performance](docs/performance/README.md) and [profiling](docs/profiling.md)
- [Browser playground](docs/playground.md), [releases](docs/release.md) and the [changelog](CHANGELOG.md)

## Development

```sh
git clone https://github.com/pocket-nexus/quickjs-oxide.git
cd quickjs-oxide
cargo run --quiet --bin qjs -- examples/module-42.mjs  # 42
cargo test --locked --workspace --all-targets
./scripts/test262/test-test262.sh --check
./scripts/test262/test-test262.sh --focused
TEST262_WORKERS=2 ./scripts/test262/test-test262.sh --full
./scripts/web/test-web-playground.sh
npm ci && npx playwright install chromium && npm run test:browser
```

`rust-toolchain.toml` pins the toolchain. The core (bytecode, heap, loop and
builtins) lives in the `quickjs-oxide-core` crate (`core/`), the front end, the
register lowering and the embedding API in the root crate (`src/`), the
regular-expression engine and Unicode tables in the `quickjs-oxide-regexp` crate
(`regexp/`), host adapters in `adapters/`, the CLI and WebAssembly apps in
`apps/`, and the Test262 runner in `conformance/test262`; the
[architecture guide](docs/architecture.md) describes each crate's
responsibilities.

## License

[MIT](LICENSE). Third-party notices are in [NOTICE](NOTICE) and
[LICENSES](LICENSES/).
