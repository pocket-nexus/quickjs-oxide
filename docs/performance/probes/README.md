# Diagnostic probes

## Fixed workload matrix

[`build_fixed_matrix.py`](build_fixed_matrix.py) generates 20 project-owned JavaScript programs and [`fixed/manifest.json`](fixed/manifest.json). The manifest stores each source's SHA-256 and expected stdout, so `scripts/benchmark/fixed.py` can compare two engine builds under the same guest workload.

| Path | Case |
| --- | --- |
| Numeric local computation and conversion | `numeric_local_loop`, `numeric_coercion_loop` |
| Ordinary local reads | `local_read_with_setup`, `local_read` |
| Value and object operations | `bigint_32/64/256`, `prop_read/write`, `array_read/write`, `call0`, `string_bridge` |
| Binding writes | `local_move`, `argument_move`, `number_owner_fallback`, `object_move`, `empty_loop` |
| Exception semantics | `type_error`, `tdz` |

The generator reads the binding-write programs from [`ordinary-writes/`](ordinary-writes/) and produces the other programs from templates. Every item validates its result against a fixed output. `--check` verifies that the bytes in `fixed/` match the generated output; `--smoke` uses eight loop iterations; `--oracle node` checks the generator's declared outputs with Node.

```sh
python3 docs/performance/probes/build_fixed_matrix.py --check
python3 docs/performance/probes/build_fixed_matrix.py --smoke --output "$OUTPUT/smoke" --oracle node
python3 scripts/benchmark/fixed.py --manifest docs/performance/probes/fixed/manifest.json \
  --workload-dir docs/performance/probes/fixed \
  --engine reference="$REFERENCE_QJS" --engine candidate="$CANDIDATE_QJS" \
  --repeat 8 --order abba-baab --output "$OUTPUT/fixed"
```

Formal measurements use plain release builds and record source, binary and workload identity for both sides. Logical completion and handoff counts are collected from a separate profiling build. Sampling and cost fields are described in the [measurement method](../measurement.md).

## Callgrind probes

[`allocation/ledger.py`](allocation/ledger.py) generates object-literal, `new` and empty-loop probes and, at N, 2N and 4N, uses Callgrind call-chain context to split per-iteration instructions into mutually exclusive phases and mechanisms.

[`runtime/probes.py`](runtime/probes.py) adds binding, property, array, call and construction probes to those three (`s=o`, `s=o.x`, `o.x=i`, `o.x=p`, `a[i&1023]=i`, `a[i&15]=p`, `f(i)`, `o.m(i)`, `g(p)`, `new E()`). Each probe validates its output and reports, from the N/2N/4N slope, per-iteration Ir, the net value after subtracting the empty loop, and the 2N−N difference of per-function self cost; on aarch64 inlining distorts the call-chain phase split, so the per-function self difference is the primary split. `--v8` adds fixed-iteration V8 v7 cases and reports Ir/Dw, the refcount self share and per-function self cost; `--v8-case CASE[=ITERATIONS]` selects cases and iteration counts.

```sh
python3 docs/performance/probes/runtime/probes.py --qjs "$CANDIDATE_QJS" \
  --v8 "$V8_V7_DIR" --output "$OUTPUT/probes"
```

[`runtime/classify.py`](runtime/classify.py) groups the per-function self cost of the V8 cases in a `probes.py` report into read cache, dense array, property write, call and return, release/GC/allocation, RegExp and builtins, and interpreter loop and numeric classes; `--other` lists the largest unclassified functions.

[`runtime/ic_events.py`](runtime/ic_events.py) runs eight V8 cases on a profiling build, counts read-cache event kinds (monomorphic and polymorphic first-entry/later-entry hits and their prototype hits, guard failures, over-capacity skips, accessor sites, cold sites, misses) and writes `ic.json`.

```sh
python3 docs/performance/probes/runtime/classify.py "$OUTPUT/probes/probes.json"
python3 docs/performance/probes/runtime/ic_events.py "$PROFILING_QJS" "$V8_V7_DIR" "$OUTPUT/ic"
```

## New-core probes

[`new-core/phase1.py`](new-core/phase1.py) runs the Rust heap workloads of the new core (`core/examples/workloads.rs`) and their current-core JavaScript counterparts at N and 2N under `perf stat -e instructions:u`, subtracts each engine's empty loop and reports the per-iteration slope. The results are in [New core, phase 1 results](../new-core-phase1-results.md).

```sh
cargo build --release -p oxide-core --example workloads
python3 docs/performance/probes/new-core/phase1.py \
    --workloads target/release/examples/workloads --qjs "$CANDIDATE_QJS" --cpu 2 --repeat 5
```
