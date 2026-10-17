# New core, phase 6: the switch and the wrap-up

This page is the implementation plan for phase 6 of the new core tracked in issue #122 ("delete the old core, old arenas and publication-time validators; re-measure code size; rewrite docs", exit: "non-test line count reduction against the 260k baseline recorded"). Phase 4 §13 asked for #122's size S to be re-estimated as M: before anything can be deleted, the public embedding API, the `qjs` command line, the web playground, the adapters, the Test262 runner and the differential corpora have to run on the new core, and that work is here. **This phase runs before phase 5** (the maintainer's decision of 2026-10-17): nothing in phase 5's memory and embedding work has a consumer today, every one of its items is cheaper once a single engine is left in the tree, and the API this phase fixes is what those items later plug into. Phase 5 is then taken item by item ([phase 5 plan](new-core-phase5.md), section 8). Like the earlier plans it is written for whoever implements the phase: it fixes the decisions that are made, lists the ones that are deliberately open with the milestone that decides each, and gives every milestone an acceptance check that can be run without judgment calls. Day estimates order the work; they are not commitments.

Read first: [#122](https://github.com/pocket-nexus/quickjs-oxide/issues/122), the [phase 4 results](new-core-phase4-results.md) for Test262's exit number and the dropped-variant list, [phase 0](new-core-phase0.md) §2 for the five dropped features that become deviation-ledger entries in this phase, [`core/README.md`](../../core/README.md), [`src/engine/api/README.md`](../../src/engine/api/README.md) and [`docs/architecture.md`](../architecture.md) for what is being replaced, [`docs/release.md`](../release.md) for how a release is cut, and [Measurement method](measurement.md).

## 0. Scope

**In scope.** The embedding API over `oxide_core::Runtime`; every in-tree consumer switched to it (`qjs`, `apps/web`, `adapters/native`, `adapters/web`, `conformance/test262`, the oracle and fixture corpora, the benchmark tools); the deletion of the old core and of everything only it used; the five deviation-ledger entries; the documentation rewrite; the line-count, binary-size and build-time record; release 0.3.0. Two small items taken over from phase 5 because the API needs them: `RuntimeOptions` and `MemoryReport` (2.4).

**Out of scope.** Any new engine behavior. Any performance work: this phase's measurement is a replay, and a change in a replayed number is a defect to find, not a result to record. Redesigning the compiler's stack IR (the lowering consumes it as phase 2 left it; a direct lowering from the compiler's own IR is a later project). Folding `oxide-core` into the root crate: it stays a crate (section 9). Everything else of phase 5: the cap as a policy, reclaiming dynamic code, images and snapshots, the 32-bit and `no_std` builds, the generational collector.

**The baseline this phase counts against.** #122's "260k" was measured before phase 1 by a method the issue does not state. This page fixes the method and the number. *Non-test lines* are the lines of every `.rs` file that is not under a `tests/` directory and is not named `tests.rs` or `*_tests.rs`; it is the method phase 4 used for "86,769 non-test lines in `src/engine/builtins`" (the same count on this tree gives 86,993). On `core/phase4` at `c2d8345b`:

| Crate or module | Non-test lines |
| --- | ---: |
| root crate `src/` | 290,802 |
| of which `engine/builtins` | 86,993 |
| `engine/vm` | 70,898 |
| `engine/object` | 35,634 |
| `engine/compiler` | 34,997 |
| `engine/heap` | 23,482 |
| `engine/value` | 9,771 |
| `engine/code` | 9,056 |
| `engine/api` | 7,315 |
| `engine/modules` | 6,301 |
| `engine/atom` | 2,244 |
| `engine/realm` | 1,641 |
| `engine/jobs`, `engine/host`, `engine/*.rs` | 1,220 |
| `source/`, `lib.rs`, `regexp.rs` | about 1,250 |
| `core/` (`oxide-core`) | 41,883 |
| `regexp/` (`oxide-regexp`) | 12,169 |
| `conformance/test262` | 13,861 |
| `apps/cli`, `apps/web`, `adapters/*` | 2,501 |
| **workspace** | **361,216** |

M0 commits the counting script and re-records this table on `main` at the phase's start; the exit compares against that record, and #122's figure is superseded. The old core (`heap`, `vm`, `object`, `builtins`, `realm`, `value`, `atom`, `jobs`, the old `api` and `host`, most of `code`) is about 250,000 of the root crate's 290,802 lines; the root crate after this phase is the front end, the lowering and the new API, projected at 45,000–55,000 lines, and the workspace at about 115,000–125,000. Those are projections; the exit records the measured numbers.

## 1. Location and order

The order is the one that keeps every gate green at every pull request: build the new API beside the old one, move consumers over one at a time, each behind its own tests, delete the old core in one pull request once nothing depends on it, then rewrite the documentation and record the counts.

| Step | Where | Gate that proves it |
| --- | --- | --- |
| the API over the new core | `src/engine/api` grows a module `core` (the current `api::new_core` generalized), exporting the types of section 2 | the API's own test suite (section 2.6), the two-core corpus through the API |
| the web playground | `apps/web`, `adapters/web` | `scripts/web/test-web-playground.sh` |
| the Test262 runner | `conformance/test262`, `dev-support/test262/*.conf`, `scripts/test262` | the new core's focused and full receipts replay as `current.conf` |
| `qjs` and the native adapter | `apps/cli`, `adapters/native` | `apps/cli/tests` (CLI, oracle, fixtures), `test-quickjs-fixtures.sh`, `test-parity-slice.sh` |
| the benchmark tools | `scripts/benchmark`, `docs/performance/probes` | Tables A and B replayed on `qjs` |
| the deletion | `src/engine/*` (section 4) | `cargo test --locked --workspace --all-targets`, clippy with `-D warnings` (dead code is a warning), the layout gates |
| the documentation | `docs/`, `core/README.md`, `README.md`, crate READMEs | `current-test262-metrics.mjs --check-docs`, the link check of `check-source-layout.py` |
| the record and the release | `docs/performance/new-core-phase6-results.md`, the `v0.3.0` tag | section 8 |

Delivery: one pull request per milestone, stacked in milestone order, gated by `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `cargo test --locked --workspace --all-targets`, `cargo test -p oxide-core --features gc-stress`, `check-new-core-stress.py`, and, for every pull request from M2 on, the Test262 focused vector of whichever spec file is current. The instruction gate of phase 4 §10 (`instructions:u` of the eight V8 v7 programs within 2% of phase 4's `oxide-run`, `c2d8345b`, sha256 `7756fa4df14df0ba…`) applies to M4, the deletion, since it changes the binary `qjs` is built into (link order and layout, as #119 and phase 4 measured) and to nothing else.

## 2. The embedding API over the new core

The current API (`Runtime`, `Context`, `Value`, `JsString`, `Error`, `RuntimeError`, `HostServices`, `ModuleLoader`, jobs, `GcPolicy`/`GcStats`/`HeapCounts`, `DebugInfoMode`, profiling, the Test262 host) is 7,315 lines over an `Rc<RefCell<RuntimeState>>`. The new core threads `&mut Runtime` and has no interior mutability. The API is the one place that reintroduces a cell, at its boundary only.

### 2.1 Shape

```rust
pub struct Runtime(Rc<RefCell<Inner>>);        // Inner: oxide_core::Runtime + the compile host + the module loader
pub struct Context { runtime: Runtime }         // the one realm (phase 0: one per runtime)
pub struct Value { runtime: Runtime, handle: oxide_core::Handle }   // released on Drop
```

- **`Runtime::new()`**, **`Runtime::with_options(RuntimeOptions)`** (2.4) and **`Runtime::with_host(impl Host, RuntimeOptions)`**. Every method takes `&self` and borrows the cell for the duration of one engine entry. The engine never calls the API from inside itself (natives reach the host through `Services`, not through `api`), so the borrow conflicts only when an embedder re-enters the API from a host callback; a callback receives a `Scope<'_>` (2.3) for that, and a direct re-entry returns `RuntimeError::Reentered` rather than panicking.
- **`Context`.** `Runtime::context()` returns the realm's `Context`; `Context::new(&runtime)` is kept for source compatibility and returns the same realm, and a second realm is `RuntimeError::Unsupported("realms")` (phase 0). `Context` keeps the methods consumers use: `eval*`, `compile*`, `execute`, `call`, `construct`, `construct_with_new_target`, `new_object`, `new_object_with_prototype`, `new_array`, `new_array_from_values`, `get_property`, `set_property`, `get_own_property`, `define_own_property`, the `*_prototype` intrinsics, `global_object`, `has_exception`, `take_exception`, `new_native_error`. Dropped: `global_var_object` (no separate var object), `create_global_lexical_for_test` and `initialize_global_lexical_for_test` (the lexical environment is the core's), `try_clone` (a `Context` is `Clone`).
- **`Value`** owns a handle in the core's handle table; `Drop` releases it. `Clone` creates a second handle; when the cell is borrowed (a clone inside a callback without its `Scope`), the clone is deferred into a pending list the next entry drains, and `Drop` is deferred the same way, so neither can fail. `try_clone` stays as an alias returning `Ok`. The conversions stay (`to_js_string`, `to_number`, `to_boolean`, `strict_equal`, `same_value*`, `number`) over `Runtime::display`-style entries in the core; `JsString` becomes a Rust `String` plus the lossless `Vec<u16>` form (`units()`), since the core's strings are one-byte or two-byte records, not the old `JsString`.
- **Errors.** `RuntimeError::{Reentered, Unsupported, Compile(Error), Thrown(Value), OutOfMemory}`; `Error`/`ErrorKind` stay for the front end's diagnostics (type, message, line, column, as the Test262 negative contracts need them). `RuntimeError::Poisoned` and `Runtime::is_poisoned` are gone: the new core has no quarantine; a panic in the engine is a panic the embedder's thread sees, as the CLI's evaluation thread already handles.

### 2.2 Host services

The current `HostServices` (`write_output`, `now_millis`, `timezone_offset_minutes`, `random_seed`) and the core's `Host` (`write`, `now_ms`, `local_tz_offset_minutes`, `read_file`) become **one trait, `oxide_core::host::Host`**, re-exported by the API:

| Method | From | Note |
| --- | --- | --- |
| `write(stream, text)` | core | `print`, `console.log`, the qjs helpers |
| `now_ms()` | both | |
| `local_tz_offset_minutes(utc_ms)` | both | |
| `random_seed() -> u64` | current | the initial `Math.random` state; the core's default derives it from `now_ms` as QuickJS seeds from host time |
| `read_file(path)` | core | `load`, `read` under `--std` |
| `promise_rejection(event, promise: Value)` | new | `HostPromiseRejectionTracker`; the core gains the hook (phase 4 §6.3 sketched it and phase 4 did not need it), with `Rejected` and `Handled` events, so `qjs --no-unhandled-rejection` keeps its meaning |

`adapters/native`'s `SystemHostServices` becomes `SystemHost` implementing it over the system clock and `tz-rs`; `adapters/web`'s over `js_sys::Date`. The module loader stays a separate trait installed with `Runtime::set_module_loader`, because consumers set it independently of the host:

```rust
pub trait ModuleLoader {
    fn normalize(&mut self, scope: &mut Scope<'_>, referrer: &str, specifier: &str, attributes: &ModuleImportAttributes) -> Result<String, Value>;
    fn load(&mut self, scope: &mut Scope<'_>, name: &str, attributes: &ModuleImportAttributes) -> Result<ModuleSource, Value>;
}
```

It is the current contract (`normalize`, `check_attributes` folded into `load`, `load`) with the requesting `Context` replaced by a `Scope`, and it is installed into the core's `ModuleHost` as `api::new_core` installs the exact-module loader today, so the runner's admissions carry over unchanged.

### 2.3 `Scope`: the callback's view of the runtime

A host callback runs while the API holds the cell's borrow. It receives `&mut Scope<'_>`, a reborrow of the inner runtime for the callback's duration, with the subset of `Context` it needs: creating strings, objects and errors, compiling a module source, reading and converting values, and throwing (`Err(Value)`). A `Value` created through a `Scope` is handle-rooted like any other and outlives the callback. This is the "embedder's handle shape" phase 1 §6 deferred: scoped reborrows for callbacks, explicit handles for everything else.

### 2.4 Jobs, collection, debugging

- `Runtime::run_pending_job() -> Result<PendingJobOutcome, RuntimeError>` runs one job; `Runtime::run_jobs()` drains. `with_execution_turn` keeps its meaning for the kept-objects list (deviation ECMASCRIPT-WEAKREF-LIFETIME-001, which the new core keeps): entries inside a turn do not clear it, the turn's end does.
- `GcPolicy::{Automatic, Manual}`: `Manual` disables the growth trigger (collections only at the cap and on `run_gc()`); `GcStats` is the core's; `HeapCounts` becomes `MemoryReport`, below.
- `DebugInfoMode::{Full, Stripped}` maps onto the unit's positions and its stored source text (`Stripped` stores neither, and `Function.prototype.toString` then prints the native form, as QuickJS does for stripped code); `set_recursion_limit` becomes the native-stack budget of `RuntimeOptions`; `set_can_block` stays (the `Atomics.wait` policy the Test262 worker sets); `declare_thread_stack` stays (the front end and the core's native-stack budget both measure the thread's stack).
- The profiling API (`new_with_allocation_trace`, `memory_snapshot`, the `oxide-memory-v1`, `oxide-allocation-trace-v1`, `oxide-compile-vm-cost-v2` and `oxide-lifecycle-v1` JSONL records, `apps/cli/src/profiling.rs`, the `profiling` feature) is **removed**. Its records describe the old heap (owner retention, zero-count queues, arena backing-storage events) and have no counterpart; `--gc-stats`, `--ic-stats` and `--memory-report` are the replacement, and `docs/profiling.md` is rewritten around them (section 6).
- **`RuntimeOptions`** replaces `Runtime::new(HeapLimits)` in the core and is the API's configuration type; `Default` is today's behavior exactly:

```rust
pub struct RuntimeOptions {
    pub heap: HeapLimits,            // max_bytes; usize::MAX for no cap (today's)
    pub min_heap: usize,             // 4 MiB
    pub growth: f64,                 // 2.0
    pub stack_limit: u32,            // registers; 2^20 today (DEFAULT_STACK_LIMIT)
    pub native_stack_budget: usize,  // bytes of host stack re-entrant calls may use; 1 MiB today
}
```

  Phase 5 adds behavior behind the fields (the cap-aware trigger, shrinking); this phase only threads them to where the constants are today, so Tables A and B cannot move.
- **`MemoryReport`** (`Runtime::memory_report()`) replaces `HeapCounts` and the profiling build's memory view: per arena (objects, shapes, strings, symbols, bigints, cells, functions, bytecode, internals) live records, capacity and bytes reserved; the value heap's slots in use, free slots by size class and capacity; rope bytes; atoms; the code store's units, `Insn`, position and source bytes; regular-expression programs; the register file's capacity and high-water mark; inline-cache entries; `GcStats`, the current threshold and the cap. It costs a walk of the bitmaps and free lists, never a collection, and `Heap::bytes()` (what the cap compares against) equals its reserved total. `qjs --memory-report` prints it at exit as `--gc-stats` prints the collector's. It is phase 5's measurement base and lands here because `--gc-stats` alone does not replace what the profiling build reported.

### 2.5 Test262 and testing support

`api::new_core::test262` (the `$262` host, the agents, the exact-module loader wiring, `thrown_summary`) becomes `api::test262` behind the existing `test262-host` feature; `Test262AgentSession` is the core's agent protocol. `test-support` keeps what `apps/cli`'s tests use and nothing else; `checked-handles` and `dropped-features-probe` are deleted with the old engine (the probe's one run is done and its result is frozen).

### 2.6 Acceptance (M1)

The API's test suite: every method of 2.1–2.4 exercised through the public surface, including a module loader that compiles through its `Scope`, a `Value` cloned and dropped inside a callback, a second `Context::new` returning `Unsupported`, `OutOfMemory` surfacing as `RuntimeError::OutOfMemory` under a cap, a promise rejection reaching the host, and a `MemoryReport` whose live counts and bytes match a known object graph exactly, with `Heap::bytes()` equal to its reserved total under `gc-stress`. The two-core corpus (44 programs) runs through `Context::eval` and prints what `oxide-run` prints. `cargo test -p quickjs-oxide --features test262-host --lib --bins` passes. The layout gate's public-API boundary check (`check-source-layout.py`) is updated to the new module and passes.

### 2.7 Amendments from the M0 audit

M0's import audit (the results page, M0) found that the consumers that survive this phase use a large part of the current API that 2.1–2.6 neither keep nor replace: about 1,335 sites match or build `Value`'s variants, about 240 depend on `RuntimeError::Exception` and `take_exception`, and the oracle tests use the object, property-key and descriptor types throughout. These amendments supersede 2.1–2.6 where they differ. Their rule: **the new API keeps the current API's shapes wherever the new core can give them their meaning**, so that M3 ports the consumers by changing what they link against, not how they are written; only what the new core cannot have (a second realm, the old heap's node counts, the profiling records) goes.

| Current item | After phase 6 |
| --- | --- |
| `Value` (an opaque handle in 2.1) | **the current enum stays**: `Undefined`, `Null`, `Bool`, `Int`, `Float`, `BigInt(JsBigInt)`, `String(JsString)`, `Symbol(SymbolRef)`, `Object(ObjectRef)`. Primitives are plain Rust data copied out of the core; `ObjectRef` and `SymbolRef` own a handle in the core's handle table, released on drop (2.1's deferred release applies to them). `PartialEq` compares primitives by value and references by identity; `Debug`, `try_clone` (returning `Ok`), `as_number`, `number`, `to_boolean` stay |
| `ObjectRef`, `CallableRef`, `SymbolRef` | kept; `CallableRef` is an `ObjectRef` checked callable, with `as_object` |
| `RuntimeError::Exception`, `Context::{has_exception, take_exception}` | kept: a thrown value is the context's pending exception, as now. 2.1's `Thrown(Value)` is not added. `RuntimeError` is `{Exception, Reentered, Unsupported(&'static str), Compile(Error), OutOfMemory, Invariant(&'static str), Internal(String)}`, `Display` and `PartialEq` |
| a second `new_context` on one runtime | `Runtime::new_context()` and `Context::new` return the runtime's one realm (2.1). Oracle tests that need two realms move to two runtimes when what they check is isolation, and are deleted under `NEWCORE-REALMS-001` when what they check is cross-realm behavior (M3 lists them). `Context::{id, realm_id}`, `ContextId` and `PendingJobOutcome::Executed { context }` go; `PendingJobOutcome` is `{Executed, Empty}` |
| `PropertyKey`, `intern_property_key[_js_string]`, `property_key_to_js_string` | kept: `PropertyKey` is `String(JsString)`, `Symbol(SymbolRef)` or `Index(u32)`, made by the same `Runtime` methods |
| `OrdinaryPropertyDescriptor`, `DescriptorField`, `AccessorValue`, `CompleteOrdinaryPropertyDescriptor` | kept, over `Value` |
| `Runtime::{get_prototype_of, as_callable, define_own_property, get_own_property, delete_property, has_own_property, own_property_keys, is_constructor, is_array_object, is_error_object, new_symbol, symbol_description, well_known_symbol}`, `WellKnownSymbol` | kept on `Runtime` (each is one engine entry) |
| `raw_string_property_for_diagnostics` (apps/web) | replaced by the value printer below |
| `is_job_pending`, `promise_snapshot`, `PromiseSnapshot`, `PromiseState` | kept |
| `set_host_promise_rejection_tracker` and the reason it reports | replaced by `Runtime::set_track_rejections(bool)` and `Runtime::take_unhandled_rejections() -> Vec<(ObjectRef, Value)>` (promise and reason, in rejection order, handled ones removed), over the core's tracking; `Host::promise_rejection` (2.2) is the notification hook beside it |
| `qjs_print_value_bytes` | `Runtime::print_value(&Value) -> String`: QuickJS's `JS_PrintValue`, ported into the core (no accessor, trap or coercion runs), which the core's `print` and `console.log` also use, as QuickJS's do |
| `install_qjs_helpers_with_script_args` | `Context::install_qjs_helpers(&QjsHelpers { std, script_args })`: `print`, `console.log`, `scriptArgs`, and `load`/`read` under `std` |
| `get_module_import_meta`, `execute_module`, `ModuleLoadResult::*`, `ModuleImportMetaProperty` | `Context::compile_module*` and `Context::execute_module` stay; `import.meta` properties are given by the loader or by the module's compile options. The loader's `ModuleSource` is `{Source { text, import_meta }, Json(text), Json5(text), Compiled(ModuleRef)}` |
| `quickjs_detect_module_bytes`, `number_to_string`, `declare_thread_stack` | kept at the API root (the front end's) |
| `JsString` | kept as an owned UTF-16 string with the current constructors (`try_from_utf8`, `try_from_utf16`, `try_from_bytes`, `MAX_LEN`), `len`, `code_unit_at`, `utf16_units`, `to_utf8_lossy` and the WTF-8 and CESU-8 encoders; `try_concat` and the rope probes go with the old strings |
| `JsBigInt` | kept as an owned arbitrary-precision integer with `Display`, the arithmetic the tests use and `to_f64`, over the core's `Big` |
| `HeapCounts` | `MemoryReport` (2.4); `context_nodes` and `var_ref_nodes` have no counterpart (one realm; closures capture scopes, which are objects), and the tests that read them assert on `MemoryReport`'s live counts instead |
| `DebugInfoMode::{Full, StripSource, StripDebug}` | kept as they are (2.4's two-mode form is not adopted): `qjs --strip-source` keeps positions and drops source text, as QuickJS's `JS_STRIP_SOURCE` does |
| `Context::runtime()` | kept |
| `quickjs_oxide::regexp` | removed; its five oracle files depend on `oxide-regexp` directly |
| `install_test262_host`, `new_test262_gc_function`, `detach_array_buffer` on `Context` | kept behind `test262-host` |
| `testing::{parse_int, parse_float}`, the lexer and the formatters | kept in `test-support`, from where the front end keeps them |
| adapters/native's `engine-test-support` | kept, re-exporting `Host` |
| `--disassemble`, `--gc-stats`, `--ic-stats`, `--std` | `Runtime::{disassemble(source, filename), gc_report(), ic_report(worst), memory_report()}` and `QjsHelpers::std` |
| the `profiling` builds of `build.py` and `build_compile_probe.py` | removed with the feature (3.4) |

## 3. The consumers

### 3.1 `qjs`

`qjs` is built on the new API; `oxide-run` is deleted and its tests (`apps/cli/tests/oxide_run.rs`) move to `cli.rs` under the `qjs` names. The option table after the switch:

| Kept from `qjs` | Taken from `oxide-run` | Dropped |
| --- | --- | --- |
| `-e`/`--eval`, `-m`/`--module`, `--script`, `-` (stdin), `--print-result`, `--quit`, `--no-unhandled-rejection`, `--strip-source`, `--version`, `--help` | `--std`, `--gc-stats`, `--heap-cap-mib`, `--disassemble`, `--ic-stats` (feature `ic-stats`), `--gc-stress=N` (feature `gc-stress`), `--test262` and `--cannot-block` (feature `test262-host`); new with 2.4: `--memory-report`, `--min-heap-mib`, `--gc-growth`, `--stack-limit` | `--dump`, `--trace`, `--profile-events`, `--profile-iterations`, `--profile-json`, `--profile-output` (the profiling build) |

The evaluation thread (`run_on_evaluation_stack`, 32 MiB, `declare_thread_stack`) stays. The Cargo features of the CLI become `test262-host`, `gc-stress`, `ic-stats`; the root crate's `new-core-*` features lose their prefix. `cargo binstall` metadata and `release.yml` are unchanged except for the smoke expression, which stays.

**Acceptance (M3).** `apps/cli/tests/cli.rs` passes with the new option table; the 431 files of the oracle corpus (`apps/cli/tests/oracle`) run against the pinned QuickJS with every mismatch either absent or attributed to one of the five dropped features and listed in `quickjs-fixture-gates.tsv` with the ledger entry's identifier (section 5); `test-quickjs-fixtures.sh --all`, `test-quickjs-c-oracles.sh` and `test-parity-slice.sh` pass the same way; `qjs` prints the manifest's stdout for all 37 suite programs.

### 3.2 The web playground

`apps/web`'s `evaluate` and `engine_metadata` run on the new API over `adapters/web`'s host; the `profile.web` release profile and `scripts/web/build-web-playground.sh` are unchanged. `wasm32-unknown-unknown` has no threads and no system clock: the agents are only ever started by the Test262 host, which the playground does not build, and the clock comes from the host as today. **Acceptance (M2):** `scripts/web/test-web-playground.sh`; the package size recorded beside the current one.

### 3.3 The Test262 runner

`run-test262` keeps one backend, the new core's, and loses `--engine` and `TEST262_ENGINE`; `test-test262.sh` loses the `engine` key. `dev-support/test262/current.conf` takes over the new core's receipts, profile and hashes from `new-core.conf`, which is deleted; `new-core-dropped-variants.tsv` stays as the evidence the ledger entries cite. The metrics blocks in `docs/status.md`, `docs/compatibility.md` and the README regenerate from the new receipts (`current-test262-metrics.mjs`): **81,445 of 83,562 variants in QuickJS 2026-06-04's scope (97.47%)**; the 1,866 variants of phase 4's difference list fail by decision (all in `D`), and the rest of the gap is the known failures QuickJS shares and the 208 in-scope variants the runner does not run, as before. The CI jobs `test262-focused`, `test262-full`, `new-core-test262-full` collapse to the first two over the one spec file; `new-core-suite` becomes `suite-stress`.

**Acceptance (M2).** The focused and full vectors replay from the migrated `current.conf`; `test-test262.sh --check` and `--runner-provenance` pass; `current-test262-metrics.mjs --check-docs` passes with the regenerated numbers; `check-test262-host-boundary.sh` and `test262-host-metadata.py` pass with `oxide_run` removed from the CLI's integration targets.

### 3.4 The benchmark tools and probes

`fixed.py`, `build.py`, `suite_matrix.py` and `probes/new-core/phase2.py` run `qjs` where they ran `oxide-run`; `phase2.py --engine current` goes away with the engine. `docs/performance/probes` keep their workloads. **Acceptance (M3):** Tables A and B replay on `qjs` built from the switched tree, against the baseline's binaries.

## 4. The deletion

One pull request (M4), after M1–M3 leave nothing depending on the old core:

| Deleted | Lines (non-test) | Note |
| --- | ---: | --- |
| `src/engine/builtins` | 86,993 | |
| `src/engine/vm` | 70,898 | |
| `src/engine/object` | 35,634 | |
| `src/engine/heap` | 23,482 | the old arenas and the cycle collector |
| `src/engine/value` | 9,771 | `JsValue`, `JsString`, `JsBigInt`; the number-to-string algorithms the front end needs move to `compiler` |
| `src/engine/code` except what the lowering reads | about 7,000 | `exec.rs` (the `ExecCode` encoder and fast entries), `rooted.rs`, the published-function records; `UnlinkedFunction`, `UnlinkedModule`, `Instruction`, `compute_stack_size`, `DebugInfoMode` and the metadata types move to `compiler` (the lowering's ten imports) |
| `src/engine/api` (old), `src/engine/host`, `realm`, `atom`, `jobs`, `modules`, `hash.rs`, `numeric_region_miss.rs`, `test_support.rs` | about 18,700 | `modules` (6,301): the loading, linking and evaluation algorithms are the core's since phase 4; what the front end keeps of it (the import-attribute types the loader contract names) moves to `compiler` |
| `src/engine/compiler`: numeric-region selection (`flow.rs` and its data-flow and effect analysis), the publication of fast entries, anything `-D warnings` reports dead after the above | measured | the stack-code emission, resolution and scope analysis stay: the lowering consumes them |
| `src/regexp.rs` | about 100 | the front end validates regular-expression literals through `oxide_regexp::compile` over `Vec<u16>` directly |
| `apps/cli/src/profiling.rs`, `apps/cli/src/oxide_run.rs`, `apps/cli/tests/profiling.rs` | about 1,000 | |
| the Cargo features `profiling`, `checked-handles`, `dropped-features-probe`, `new-core-*` | | |

What #122 called "publication-time validators" is the old `code` module's publication checks and the admission paths of the old VM; they go with it. The new core's `bytecode::validate` is not that: it is the one check between an image file and the loop (phase 5 §4.2) and stays.

`check-source-layout.py`'s owner table and the module-reachability check are rewritten for the remaining layout (`src/source`, `src/engine/compiler` with the lowering, `src/engine/api`), and the architecture doc's source-owner tree with it (section 6). After the deletion the root crate's production dependencies are `oxide-core`, `oxide-regexp`, `num-bigint` and `num-traits`; if the last two were only the old `JsBigInt`'s (the core has its own `Big`), they go too, and `check-rust-only.sh` keeps proving the product is Rust.

**Acceptance (M4).** The workspace builds and tests with `-D warnings` and no `#[allow(dead_code)]` added; `cargo tree -p quickjs-oxide -e normal` shows the dependency list above; every gate of section 1 passes; the counting script's number is recorded; the instruction gate on the eight against M3's `qjs` is within 2%.

## 5. The deviation ledger and compatibility

Phase 0 §2: each dropped feature "becomes an entry in the deviation ledger when the new core replaces the current one". M0 drafts and M2 lands five entries in `docs/deviations.md`, in the ledger's existing form (status, surface, authority, required behavior, baseline evidence, acceptance, specification):

| Entry | Behavior | Test262 evidence |
| --- | --- | --- |
| `NEWCORE-WITH-001` | `with` is an early `SyntaxError` in sloppy code, as in strict code | the 311 `with` variants of `new-core-dropped-variants.tsv` |
| `NEWCORE-UNSCOPABLES-001` | `Symbol.unscopables` exists; nothing reads it | 53 variants |
| `NEWCORE-DIRECT-EVAL-001` | `eval(src)` at a direct-eval site runs as indirect eval in the global scope | 517 variants |
| `NEWCORE-REALMS-001` | one realm per runtime; `$262.createRealm` throws `TypeError: realms are not supported` | 488 variants |
| `NEWCORE-SPECIES-001` | `Array`, `%TypedArray%`, `Promise`, `RegExp`, `ArrayBuffer` and `SharedArrayBuffer` methods construct the intrinsic; `Symbol.species` exists; nothing reads it | 528 variants |

Each cites the reconciliation of phase 4 M9 (31 variants of `D` pass on the new core and are not deviations). `TEST262-ANNEXB-EVAL-001` is retired as superseded: its surface is a function declaration inside a `with` environment, which is now a `SyntaxError` under `NEWCORE-WITH-001`; the entry stays in the ledger marked superseded with the date. `ECMASCRIPT-WEAKREF-LIFETIME-001` stays approved; the new core keeps the kept-objects behavior (phase 4 §3.4). Phase 5's device builds without a compile host add a sixth entry only when such a build exists in this repository.

`docs/compatibility.md` states the target as "QuickJS 2026-06-04's observable behavior except the five ledger entries", keeps the oracle and the Test262 definitions, and its metrics block regenerates (3.3). The `quickjs-fixture-gates.tsv` rows attributed to a dropped feature carry the entry's identifier, and `check-oracle-registry.py` verifies every attribution names an existing entry.

## 6. The documentation

Every page that describes the old core is rewritten or retired in M5; the phase plans and results pages stay as the record of how the core was built.

| Page | After M5 |
| --- | --- |
| `README.md` | the engine's description, performance against QuickJS from phase 4's Table A (the reported ratios, not the floor), the five deviations in one line with a link, installation unchanged |
| `docs/architecture.md` | the three-crate layout (`oxide-core`, `oxide-regexp`, the root crate's front end, lowering and API), the source-owner tree of what remains, the host boundary, compilation to register bytecode, the loop, the heap and the collector in the depth the current page gives the old paths |
| `docs/status.md` | the new core's surface and metrics; the verification commands with `qjs` |
| `docs/compatibility.md`, `docs/deviations.md` | section 5 |
| `docs/test262.md` | one backend; the dropped list as part of the metric's definition |
| `docs/performance/README.md` | the new core's execution model in the opening paragraph; the document list; the phase pages under a "History" heading |
| `docs/performance/roadmap.md` | retired; replaced by `execution.md`: the register loop, the cache kinds and hit paths, the iteration fast path, suspension, the native ABI, from `core/README.md`'s departures and the phase 2–4 plans |
| `docs/performance/principles.md` | rewritten as the rooting rule, the `&mut` discipline, the loop-size and instruction gates, and the measurement rule |
| `docs/performance/typed-arenas.md`, `docs/cycle-collection.md` | retired; replaced by `docs/heap.md`: arenas, the value heap, shapes and validity cells, the mark-sweep collector and its trigger, the cap, stress mode, from phase 1's design as built; phase 5 extends it |
| `docs/profiling.md` | `--gc-stats`, `--ic-stats`, `--memory-report`, `--disassemble`, and the Callgrind method of `probes/README.md` |
| `docs/playground.md`, `docs/release.md` | the option and asset changes only |
| `core/README.md` | stays the design record of the core (the rooting rule, every departure); gains the phase 6 pointer; its "Layout" table is the one `architecture.md` links to |
| `src/engine/api/README.md`, `src/engine/compiler/README.md`, `src/source` | the API's shape (section 2) and the front end's role as the lowering's input; the deleted modules' READMEs go with them |
| `CHANGELOG.md` | unchanged by policy (release notes come from pull-request titles); the 0.3.0 release notes carry the summary of section 8 |

**Acceptance (M5).** No page links to a deleted module or describes a removed path (a grep for the old module names and for `ExecCode`, `RuntimeState`, `FrameExecution`, `property_ic` and the other old identifiers across `docs/` and the READMEs returns nothing outside the phase pages and the ledger's history); `current-test262-metrics.mjs --check-docs` passes; `check-source-layout.py`'s documentation link check passes.

## 7. Milestones

| # | Content | Acceptance | Estimate |
| --- | --- | --- | --- |
| M0 | the counting script (`scripts/checks/count-lines.py`, the method of section 0) and the baseline on `main`; the five ledger entries drafted; the API's type list of section 2 reviewed against every in-tree consumer's actual use (`grep` of the public items each imports) | the baseline table committed; no consumer uses an item the list drops without a replacement named | 2–3 days |
| M1 | the API over the new core beside the old one (section 2), `RuntimeOptions` and `MemoryReport` included | section 2.6 | 12–17 days |
| M2 | the web playground, the adapters and the Test262 runner switched (3.2, 3.3); the ledger entries landed; `current.conf` migrated | the acceptances of 3.2 and 3.3; the ledger's new entries pass `check-oracle-registry.py` | 5–8 days |
| M3 | `qjs` on the new API, `oxide-run` folded in, the oracle and fixture corpora attributed, the benchmark tools switched (3.1, 3.4) | the acceptances of 3.1 and 3.4 | 6–9 days |
| M4 | the deletion (section 4); the layout gate rewritten | section 4's acceptance | 4–6 days |
| M5 | the documentation (section 6) | section 6's acceptance | 5–7 days |
| M6 | the record: Tables A, B and E replayed on `qjs`, the counts, binary size and build time; the results page; release 0.3.0 | section 8 | 3–4 days |

Total: about 37–54 working days. M1 may split into two pull requests (the types and the module loader with `Scope`); M2's runner switch is its own pull request because it migrates the receipts; M4 is always one pull request so that `git log` has a single commit where the old core ends.

## 8. Measurement, record and exit

**Tables A and B replayed.** The 37 suite programs and the 20 fixed probes, `fixed.py --repeat 5 --cpu 2 --linux-counters --timeout 900`, with QuickJS, the baseline's `qjs` (main `8e16f672`, sha256 `3cc9c95d597af1bd…`, which stays available as a binary after its source is deleted) and the final `qjs` of this phase: the floor per program and per suite against phase 4's reads (3.68 / 3.84 / 5.69, lowest 3.02).

**Table E replayed.** The migrated `current.conf`'s focused and full vectors; the same 81,445.

**Table J: size.** The counting script's table before (M0's baseline on `main`) and after (M4), per crate and module; the stripped release `qjs` binary's bytes (8,740,328 today; `oxide-run` 3,276,016); the wall time of `cargo build --release -p quickjs-oxide-cli` from a clean target directory, medians of 3, before and after; the wasm package size before and after.

**Exit condition.**

- Every in-tree consumer runs on the new core; the old core is deleted; the gates of section 1 pass; the dependency list of section 4 holds.
- Table E: the migrated receipts replay; the documentation metrics match them.
- The oracle corpus, the fixtures and the parity slice pass with every mismatch attributed to a ledger entry.
- Tables A and B: the floor holds per program and per suite; no V8 v7 program retires more than 2% more instructions than M3's `qjs`; `call0` and `array_write` are within 2% of phase 4's slopes (554 and 246).
- Table J committed; the line-count reduction stated against M0's baseline.
- The documentation acceptance of section 6; the five ledger entries and the retired one.
- `v0.3.0` cut by `cut-release.sh`, its notes leading with the user-visible changes: the performance table against QuickJS, the five dropped features, the memory options, the removed `--dump`/`--trace`/`--profile-*`, the API changes (one realm, `Host`, `Scope`, `Value` handles, the removed profiling API, `RuntimeError` variants), the minimum Rust version.

A replayed number that moves outside its tolerance is a defect this phase fixes before M6 ends, since the phase adds no engine behavior; the results page records the fix.

## 9. Decisions

Made, and not reopened inside phase 6:

- this phase runs before phase 5, which is then taken item by item (section 0);
- `oxide-core` stays a separate crate, answering phase 1's open question: `cargo test -p oxide-core` builds in seconds, `forbid(unsafe_code)` is a crate-level assertion, and phase 5's `no_std` build needs the boundary; the root crate is the front end, the lowering and the API;
- `RuntimeOptions` and `MemoryReport` are built here, with no behavior behind the new option fields (2.4);
- one `Rc<RefCell<…>>` at the API boundary and nowhere else; callbacks receive a `Scope`, not a `Context`; re-entry is an error, not a panic (2.1, 2.3);
- one realm per runtime; `Context` stays as the realm's handle (2.1);
- one `Host` trait for the API and the core; the module loader stays a separate trait (2.2);
- `Value` owns a handle; deferred clone and release when the cell is borrowed (2.1);
- the profiling API and build are removed, not re-targeted (2.4);
- `oxide-run` is folded into `qjs` and deleted (3.1);
- the Test262 runner keeps one backend and `current.conf` takes the new core's receipts (3.3);
- the five ledger entries and the retirement of `TEST262-ANNEXB-EVAL-001` (section 5);
- the line-count method and the baseline are M0's, superseding #122's figure (section 0);
- the deletion is one pull request (section 7);
- the release is 0.3.0, a minor version, as the CHANGELOG's policy for API changes before 1.0 allows.

Open, with the milestone that decides them:

- whether `num-bigint` and `num-traits` leave the root crate: M4, by what the front end still uses (BigInt literal parsing may keep them);
- the exact `Scope` method list: M1, from the module loaders in the tree (the runner's exact-module loader, the CLI's file loader, the playground's);
- whether the compiler's `flow.rs` analyses are entirely dead or partly feed the lowering's register allocation: M4, by `-D warnings`;
- the wording of the 0.3.0 notes: M6, by `release.md`'s rule.

## 10. Risks

- **An API consumer outside the tree.** pocketjs embeds the current API. Mitigation: the type list of section 2 is checked against its use at M0 (the maintainer supplies the import list), and the 0.3.0 notes carry a migration table from every removed or changed item to its replacement.
- **The runner migration.** `current.conf` is the authority for the Test262 metrics the documentation prints; a wrong migration fails `--check-docs` or, worse, passes with the wrong baseline. Mitigation: the receipts are moved, not regenerated; the pull request shows the new core's frozen full receipt hashing identically under the new spec file before anything else changes.
- **Hidden dependence on the old core in the front end.** The compiler may reach the old `value` or `code` types in places the lowering never exercised. Mitigation: M4 is driven by the compiler's own errors after the deletion; types the front end legitimately needs move to `compiler` (section 4) rather than being re-created.
- **Layout and the instruction gate.** Deleting about 250,000 lines changes the binary's layout; phase 4 measured 3–6% time swings from layout alone. Mitigation: the gate is on instructions, not time; a 2% instruction rise is investigated as a real change, a time swing with flat instructions is recorded as layout.
- **The oracle corpus.** 431 differential cases were written against QuickJS's full surface; some exercise the dropped features incidentally. Mitigation: attribution rows in the gates table, verified against existing ledger identifiers; a case that fails for any other reason is a defect.
- **Documentation debt.** Rewriting nine pages is the least gated work in the phase. Mitigation: the grep-based acceptance of section 6 is a script in M5's pull request, and `--check-docs` runs in `fast`.
- **Estimate.** M1 is the item most likely to grow, as every consumer's need surfaces there. Mitigation: M0's import audit sizes it before it starts; M2's and M3's acceptances are the only things that can send work back to M1.

## 11. After phase 6

Not planned here, recorded so that the thread is not lost: a direct lowering from the compiler's own IR to register bytecode (phase 2 went through the stack code; the stack code is now an intermediate form nothing executes); `Function.prototype.toString` source text and position tables as a separately loadable section of an image, for devices that want neither; the kind-specific `IterNext` for Map and Set, the heap-resident generator frame and the other options phase 4 §11 measured and did not build, should a consumer's profile ever show them. Phase 5's items follow this phase in the order its section 8 gives, each taken when a consumer asks for it.
