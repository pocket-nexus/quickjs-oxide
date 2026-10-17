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
