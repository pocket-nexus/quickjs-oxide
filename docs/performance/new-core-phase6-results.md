# New core, phase 6: results

This page records the measurements of [phase 6](new-core-phase6.md) of the
new core (#122): the line-count baseline and the consumer audit (M0), each
milestone's acceptance as it lands, and at the end the replayed tables and
the size record (section 8).

## M0: the baseline and the audit

### The counting method

`scripts/checks/count-lines.py` counts the non-test lines of section 0:
every `.rs` file that is not under a `tests/` directory and is not named
`tests.rs` or `*_tests.rs`, with `target/` and hidden directories never
counted. A line is a newline character, as `wc -l` counts it. The plan's
table split each file's text on newlines, which counts one more line per
file (`src/engine/builtins` has 156 counted files, and the plan's 86,993 is
this method's 86,837 plus 156); this page's numbers supersede it.
`--rev` counts a commit's tree without checking it out.

### The baseline

The phase's base is `core/phase4` at `c2d8345b`, the top of the stacked
pull requests of phases 1–4 (#135–#148), none of which `main` has merged
yet. `main` (`8e16f672`) has no new core: its root crate is 298,014
lines and its workspace 314,044. The deletion of M4 removes the old core
from the stacked tree, so the reduction is stated against the stacked base:

```
python3 scripts/checks/count-lines.py --rev c2d8345b
```

| Crate or module | Non-test lines |
| --- | ---: |
| root crate src/ | 290,303 |
| `src/engine/builtins` | 86,837 |
| `src/engine/vm` | 70,789 |
| `src/engine/object` | 35,579 |
| `src/engine/compiler` | 34,948 |
| `src/engine/heap` | 23,436 |
| `src/engine/value` | 9,756 |
| `src/engine/code` | 9,037 |
| `src/engine/api` | 7,291 |
| `src/engine/modules` | 6,294 |
| `src/engine/atom` | 2,241 |
| `src/engine/realm` | 1,637 |
| `src/ (other)` | 1,319 |
| `src/engine/jobs` | 839 |
| `src/engine/*.rs` | 224 |
| `src/engine/host` | 76 |
| `core` | 41,800 |
| `conformance/test262` | 13,849 |
| `regexp` | 12,155 |
| `apps/cli` | 2,016 |
| `scripts` | 283 |
| `adapters/native` | 264 |
| `apps/web` | 185 |
| `examples` | 54 |
| `adapters/web` | 27 |
| **workspace** | **360,936** |

This table supersedes #122's "260k", whose method the issue does not state.

### The consumer audit

Every in-tree consumer of the root crate was read for the public items it
uses (`grep` of `quickjs_oxide::` and of the methods called on what it
imports), and each item was mapped to section 2. The consumers that this
phase deletes (the current engine's Test262 backend, `oxide-run`, the
profiling build) are listed but not counted as gaps.

The check failed as section 2 was written: the surviving consumers use a
large part of the current API that 2.1–2.6 neither kept nor replaced.

| Gap | Sites in surviving consumers |
| --- | ---: |
| `Value` as an enum, matched and constructed | about 1,335 (1,263 in the oracle tests) |
| `RuntimeError::Exception` with `take_exception` | about 240 |
| the descriptor types | about 220 |
| `JsString`'s checked constructors | about 215 |
| `ObjectRef` and `CallableRef`, `as_object` | about 310 |
| `Runtime::get_prototype_of` | 361 |
| a second realm on one runtime (`new_context` pairs) | 124, in 90 oracle files |
| `PropertyKey` and `intern_property_key` | about 160 |
| `well_known_symbol`, `new_symbol` | 86 |
| `HeapCounts::context_nodes`, `var_ref_nodes` | 50 |
| `is_job_pending`, `promise_snapshot` | about 45 |
| `number_to_string`, `JsBigInt` | 30 |
| `qjs`'s own entries (the value printer, the helpers, `import.meta`, `execute_module`, the module-source variants) | about 20 |
| `quickjs_oxide::regexp` | 5 files |

Section 2.7 of the plan amends section 2 with a replacement for every one of
them, by one rule: the new API keeps the current API's shapes wherever the
new core can give them their meaning. With 2.7, no surviving consumer uses
an item that is dropped without a replacement named, which is M0's
acceptance. Two consequences for later milestones:

- The new core has no JSON5 parser. `type: "json5"` modules (`qjs`'s
  loader) need one before M3's fixtures can pass; it is added to the core
  in M1 with the module-source variants.
- `qjs` prints values with QuickJS's `JS_PrintValue` (`{ a: 1 }`, stack
  traces of uncaught errors), which the new core's `print` does not do: it
  prints `ToString`. The printer is ported into the core in M1, and the
  core's `print` and `console.log` use it, as QuickJS's do.

### The base's test suite

`cargo test --locked --workspace --all-targets` on `c2d8345b` had one
failure, `oxide_run::an_unsupported_construct_is_reported`: a phase 2 test
that expected classes to be unsupported, stale since phase 4. M0 replaces it
with a test that `with` is an early `SyntaxError`; the workspace then passes.

## M1: the API over the new core

`quickjs_oxide::engine::api::core` is the embedding API of section 2 as
amended by 2.7, beside the current API. One `Rc<RefCell<…>>` holds the
core's runtime; every method borrows it for one engine entry.

| Piece | Where |
| --- | --- |
| `Runtime`, `Context`, entries, value conversion, jobs, rejections, memory, keys, symbols, object operations | `src/engine/api/core/{runtime,context}.rs` |
| `Value` (the current enum), `ObjectRef`, `SymbolRef`, `CallableRef` (handles released on drop, deferred while the runtime is borrowed) | `value.rs` |
| `PropertyKey`, the descriptor types, `WellKnownSymbol` | `property.rs` |
| `ModuleLoader`, `Scope`, `ModuleSource`, the core's module hooks | `module.rs` |
| the front end and the lowering as the compile host (shared with `oxide-run` until M3) | `compile.rs` |
| the core's embedder entries: properties, conversions, values, `run_gc` | `core/src/embed.rs` |
| `RuntimeOptions` (`Runtime::new` takes it; `HeapLimits` converts into it) | `core/src/runtime.rs` |
| `MemoryReport` and `qjs --memory-report`'s table | `core/src/heap/report.rs` |
| QuickJS's `JS_PrintValue` (`Runtime::print_value`; `print` and `console.log` use it) | `core/src/vm/print.rs` |

What the core gained for it, none of it on the loop's path:

- **Host.** `random_seed` (set from the host when it is installed) and
  `promise_rejection` with `Rejected` and `Handled` events, called where
  ECMAScript calls `HostPromiseRejectionTracker`. While tracking is on, the
  runtime keeps the unhandled rejected promises as roots for the embedder.
- **Modules.** A loader's thrown value passes through as is
  (`CompileError::thrown`); the normalizer receives the compiler, so it can
  have a `Scope`; a module source carries its `import.meta` properties; JSON5
  modules parse with QuickJS's extended grammar (a mode of the JSON parser:
  comments, single quotes, identifier keys, trailing commas, `+`, `.5`,
  hexadecimal, octal and binary numbers, `NaN`, `Infinity`). A module
  compiled again under the same name replaces the first for later imports.
- **Errors.** A compile error located in its source (`CompileError::at`)
  gives its error a `stack` that starts with `file:line:column`, as
  QuickJS's does. `eval` syntax errors now have it.
- **Jobs.** `run_pending_job` runs one job; `clear_kept_objects` ends an
  execution turn.
- **The collector.** A manual mode (no growth trigger) behind
  `GcPolicy::Manual`.
- **The printer.** An array that has had a hole is remembered in a weak side
  set and printed by its index properties, as QuickJS prints an array that
  left its fast form (the header has no free flag bit).

### Acceptance

- `tests/embedding_api.rs` exercises sections 2.1–2.4 through the public
  surface: values of every kind, pending exceptions and located syntax
  errors, the one realm, calls, construction, descriptors and keys, symbols
  and conversions, jobs, promise snapshots and tracked rejections, a host
  receiving output, time, the seed and rejection events, modules through a
  loader (source, `import.meta`, JSON, JSON5, a module compiled through the
  `Scope`, the loader's own error, a syntax error, `import()`), re-entry
  from a callback (`Reentered`) with values cloned and dropped inside it,
  `OutOfMemory` under an 8 MiB cap, the memory report of a known graph, the
  execution turn, the reports and the manual collection policy.
- `core/tests/report.rs` checks that the report never collects, that its
  live counts follow a known graph exactly and that its reserved total
  equals `Heap::bytes()`, with and without `gc-stress`; and that the options
  reach the heap and the stack.
- The 44 programs of the two-core corpus print through `Context::eval`
  exactly what the driver prints (`api::core::tests`).
- 2.6 asked for a second `Context::new` to return `Unsupported`; 2.1 and
  2.7 make it return the one realm, which the test checks instead.

### Differences the printer port found

The printer was checked against the pinned QuickJS on a probe set of 23
lines covering every value kind, nesting, cycles, accessors, collections,
typed arrays, regular expressions, dates, errors, wrappers, iterators and
functions. One difference remains: QuickJS prints an ordinary object's
properties in insertion order, and the new core keeps index keys in the
element range, so `{b: 1, 0: 2}` prints as `{ 0: 2, b: 1 }`. M3's oracle run
records whether any case meets it.

Two stack-trace gaps showed up and are left to M3, where the oracle corpus
measures them: an error the loop itself throws has no line and column in
its frame's `stack` line, and native frames (`at reduce (native)`) are not
listed.

## M2: the playground, the adapters and the Test262 runner

### The adapters and the playground

`adapters/native` gains `SystemHost` and `adapters/web` gains `WebHost`, the
new core's `Host` over the same clock, time zone and seed as the current
`SystemHostServices` and `WebHostServices` (which stay until `qjs` moves in
M3). `apps/web` evaluates on `api::core`: a fresh runtime per call, the jobs
drained, an error reported as `name: message`.

Building the core for `wasm32-unknown-unknown` for the first time found
three things, fixed here:

- `Math.sumPrecise` compared a length with `1 << 53`, which overflows a
  32-bit `usize`.
- The collector timed its pauses with `Instant::now()` and the default seed
  read `SystemTime::now()`; both panic on a target without a clock. A small
  `clock` module reports zero-length pauses and a constant seed there (the
  embedder's host seeds `Math.random` instead).
- A delegated generator resumes by re-entering the loop, so a `yield*` chain
  uses the host's stack. On WebAssembly that is the JavaScript engine's
  machine stack, which the core's byte measure (of the linear-memory stack)
  cannot see: under Node a chain overflowed V8 between 500 and 1,000 levels,
  where the current engine went to 1,000. `RuntimeOptions` gains
  `native_depth_limit` (unbounded by default), and the playground sets it to
  600 re-entries with a 512 KiB byte budget, below the 1 MiB linear-memory
  stack `wasm-ld` reserves. Under Node, with those limits, seven recursion
  patterns (`yield*`, `Array.prototype.map`, a getter, `toString`, `sort`, a
  `JSON.parse` reviver, a proxy trap) each end in a catchable
  `InternalError: stack overflow` when unbounded and reach 300–600 levels
  when bounded. The playground test's deep `yield*` chain is 400 levels
  (1,000 before).

The core's `Atomics.wait` refusal said `cannot block in this agent`; QuickJS
says `cannot block in this thread`, which the playground's example checks.

`scripts/web/test-web-playground.sh` passes. The package's WebAssembly
module, as `build-web-playground.sh` writes it to `target/pages/pkg`:

| Build | `quickjs_oxide_web_bg.wasm`, bytes |
| --- | ---: |
| `main` (`8e16f672`), the current engine | 4,978,281 |
| M2, the new core | 2,119,640 |

The stacked branches of phases 2–4 could not build the playground at all:
the root crate depends on `oxide-core`, which did not compile for a 32-bit
target until the fix above.

### The Test262 runner

`run-test262` has one backend. The new core's worker moved from
`api::new_core::test262` to `api::core::test262` and takes a `Host`; the
runner's `--engine`, `TEST262_ENGINE` and the current engine's worker (about
1,600 lines with its tests) are gone. `test-test262.sh` makes the
dropped-variant keys required and drops the `engine` key.

`current.conf` takes `new-core.conf`'s keys unchanged except the `engine`
line, and `new-core.conf` is deleted: the receipts were moved, not
regenerated. Before anything else changed, phase 4's full report (kept in
`target/`) hashed to the frozen `full_tsv_body_sha256`
(`5f1b44cc…`) under the moved spec, and `--check` authenticated it. The
previous engine's focused receipts are deleted with their spec.

### Table E replayed

| Vector | Frozen (phase 4) | Replayed (M2) |
| --- | ---: | ---: |
| focused, of 6,844 | 6,814 | 6,814, byte-identical body |
| full, QuickJS scope, of 83,562 | 81,445 | 81,445 |
| full summary | `fail-async=16 fail-async-done-count=53 fail-parse=324 fail-runtime=1516 pass=81445 …` | the same |

The full body differs in 100 rows, all of them failing direct-`eval`
variants of D (`new.target` and `super` in eval code): only the reported
line and column changed. M1 gave an `eval` syntax error QuickJS's first
`stack` line, `at <input>:line:column`, and the runner reads the location
from the first frame, so these rows now report the error's position in the
eval source (`1:6`) instead of the calling frame's (`21:21`). No outcome,
type or message changed. The full receipt's body hashes are re-frozen
(milestone `new-core-phase6-m2`) by the promotion procedure of
`docs/test262.md`.

Of the runner's 106 unit tests, 21 tested the current engine's own worker
(its module loader, its dynamic-import bytecode authentication, its
diagnostic extraction, its async module completion) and went with it. Of the
tests that drive the worker, six failed on the new core:

| Test | Finding | Change |
| --- | --- | --- |
| the `print` host surface | `print.length` was 0; QuickJS's is 1 (`console.log` too) | core fixed |
| the `$262` hosts | `$262.IsHTMLDDA.name` was empty | core fixed |
| the `$262` hosts | `$262.detachArrayBuffer` threw for a non-buffer and returned `null`; QuickJS ignores anything but an ArrayBuffer and returns `undefined` | core fixed |
| the `$262` hosts | `$262.codePointRange` converted with a saturating cast; QuickJS uses `ToUint32` | core fixed |
| the `$262` hosts | `createRealm` makes a child realm | now checks `NEWCORE-REALMS-001`'s `TypeError` |
| the `$262` hosts | a detached resizable buffer's `maxByteLength` is 0 (ECMAScript) where QuickJS keeps 8 | engine semantics, out of the host test; left to M3's oracle run |
| the async print log | the worker scanned every argument of `print` for the completion marker; the new core captures lines | one marker per call, as the harness prints it |
| unaudited dynamic import | the refusal's wording | assertion follows it |
| a parse-rejected dynamic-import root | the new core compiles `import('')`, so the root is refused as "unexpectedly compiled" | assertion follows it |
| runtime-compiled `import()` | the current worker authenticated `eval`, `Function` and `$262.evalScript` code containing `import()`; the new core's worker never did (phase 4 froze its receipts without it) | test removed; recorded as a hardening gap of the runner |

### The ledger and the metrics

The five `NEWCORE-*` entries are approved (2026-10-17) and
`TEST262-ANNEXB-EVAL-001` is marked superseded by `NEWCORE-WITH-001`.
`check-oracle-registry.py` now checks the ledger: unique identifiers, the
seven fields of every new-core entry, the five expected new-core entries,
and that every fixture attribution (a `deviation` column, which M3 adds)
names an entry. The README, `docs/status.md`, `docs/compatibility.md` and
`docs/test262.md` regenerate to **81,445 of 83,562 (97.467%)**.

CI: `test262-focused` runs the one spec; `new-core-test262-full` is gone
(`test262-full` covers it); `new-core-suite` is `suite-stress`.
