# quickjs-oxide

A JavaScript engine written entirely in safe Rust, with zero `unsafe`, that
reproduces the behavior of [QuickJS](https://bellard.org/quickjs/) 2026-06-04.

<!-- current-test262-metrics:start -->
Within the Test262 scope that QuickJS 2026-06-04 runs, quickjs-oxide passes
**83,304 of 83,562 variants (99.691%)**. QuickJS's `test262.conf`
skips the other 18,475 of the 102,037 variants; across the full corpus the
pass rate is 81.641%.
<!-- current-test262-metrics:end -->

- **Zero `unsafe`.** Every crate in the workspace sets
  `unsafe_code = "forbid"`, so the compiler rejects `unsafe` anywhere in the
  engine, the CLI and the WebAssembly bindings. The garbage collector,
  compiler, VM, regular expression engine and Unicode tables are all safe
  Rust; the only third-party code in the engine is `num-bigint` and
  `num-traits` for BigInt arithmetic.
- **Same behavior as QuickJS.** Every failing Test262 variant is one QuickJS
  2026-06-04 also fails, early errors carry QuickJS's exact messages and
  positions, and a differential suite compares the two engines case by case.
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
# or build from source with Rust 1.95 or newer
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
use quickjs_oxide::engine::api::{Runtime, Value};
use quickjs_oxide_host::SystemHostServices;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    let mut context = runtime.new_context()?;
    let value = context.eval("[1, 2, 3].map((x) => x * 2).join()")?;
    if let Value::String(text) = value {
        println!("{}", text.to_utf8_lossy()); // 2,4,6
    }
    Ok(())
}
```

`examples/eval.rs` handles every completion value and uncaught exceptions.
Host services supply time, randomness and output; module loader callbacks let
an application resolve and load ES modules.

## Compatibility

The language, built-ins and ES modules (static and dynamic `import`,
top-level `await`, import attributes, JSON modules) follow QuickJS 2026-06-04.
The `std` and `os` modules, the REPL, `Worker`, `qjsc` and the C API are not
yet supported. [Compatibility](docs/compatibility.md) details the baseline,
how conformance is measured, the approved deviations and the full product
surface.

## Documentation

- [Compatibility with QuickJS 2026-06-04](docs/compatibility.md)
- [Architecture](docs/architecture.md) and [implementation status](docs/status.md)
- [Test262 baseline and metrics](docs/test262.md)
- [Execution and performance](docs/performance/README.md) and [profiling](docs/profiling.md)
- [Browser playground](docs/playground.md) and [releases](docs/release.md)

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

`rust-toolchain.toml` pins the toolchain. The engine lives in `src/`
(`source/`, `regexp/` and `engine/`), host adapters in `adapters/`, the CLI
and WebAssembly apps in `apps/`, and the Test262 runner in
`conformance/test262`; the [architecture guide](docs/architecture.md)
describes each module's responsibilities.

## License

[MIT](LICENSE). Third-party notices are in [NOTICE](NOTICE) and
[LICENSES](LICENSES/).
