# Implementation status

quickjs-oxide is a Rust JavaScript engine available through its command-line application, embedding API, native adapter and browser playground. The current module layout and execution contracts are in the [architecture guide](architecture.md). [Execution and performance](performance/README.md) documents the implemented VM paths and their measurement.

## Engine

- The compiler parses Script and ECMAScript Module source, resolves bindings, lowers stack instructions and publishes one `ExecCode` word stream.
- The VM uses explicit frames and one instruction loop. Published numeric operations, ordinary binding transfers and selected named-property reads complete in the frame access scope when their semantic requirements are satisfied. Drivers handle callbacks, general property operations, conversion, exceptions and suspension.
- The heap owns full-generation identities, roots, reference edges, typed captured-cell and shape arenas, collection and deferred cleanup.
- The runtime implements objects, strings, numbers, regular expressions, binary data, collections, weak references, Promises, jobs, modules and the corresponding builtins.
- Host adapters supply environment capabilities. Applications own file loading, output, diagnostics and job driving.
- The CLI and browser playground execute the Rust engine. Exact source bytes, malformed string data and diagnostic positions travel through the embedding and loader APIs.

## Test262 baseline

<!-- current-test262-metrics:start -->
The authoritative R3fj Test262 vector has:

- 80,010 full-corpus passes out of 102,037 variants (78.413%)
- 80,060 eligible variants out of 102,037 (78.462%)
- 80,010 passes out of 80,060 runnable variants (99.938%, secondary quality
  metric)
- 50 classified failures and no timeouts among eligible variants
<!-- current-test262-metrics:end -->

The profile, admission vector and result hashes are recorded in [`dev-support/test262/current.conf`](../dev-support/test262/current.conf). [Test262 guidance](test262.md) explains the runner and metric definitions.

<a id="verification"></a>
## Verification

```sh
cargo test --locked --workspace --all-targets
cargo test --locked --features test262-host --lib --bins
./scripts/quickjs/test-quickjs-c-oracles.sh --check
./scripts/test262/test-test262.sh --check
./scripts/test262/test-test262.sh --focused
TEST262_WORKERS=2 ./scripts/test262/test-test262.sh --full
./scripts/web/test-web-playground.sh
```
