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
