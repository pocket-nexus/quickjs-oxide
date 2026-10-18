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

Formal measurements use plain release builds and record source, binary and workload identity for both sides. Inline-cache hit and miss counts come from a `qjs` built with the `ic-stats` feature (`qjs --ic-stats`). Sampling and cost fields are described in the [measurement method](../measurement.md).

## Callgrind probes

[`runtime/probes.py`](runtime/probes.py) runs empty-loop, object-literal and `new` probes and binding, property, array, call and construction probes (`s=o`, `s=o.x`, `o.x=i`, `o.x=p`, `a[i&1023]=i`, `a[i&15]=p`, `f(i)`, `o.m(i)`, `g(p)`, `new E()`) under Callgrind at N, 2N and 4N. Each probe validates its output and reports, from the N/2N/4N slope, per-iteration Ir, the net value after subtracting the empty loop, and the 2N−N difference of per-function self cost. `--v8` adds fixed-iteration V8 v7 cases and reports Ir/Dw and per-function self cost; `--v8-case CASE[=ITERATIONS]` selects cases and iteration counts.

```sh
python3 docs/performance/probes/runtime/probes.py --qjs "$CANDIDATE_QJS" \
  --v8 "$V8_V7_DIR" --output "$OUTPUT/probes"
```

## Instruction slopes

[`slopes/heap_workloads.py`](slopes/heap_workloads.py) runs the Rust heap workloads (`core/examples/workloads.rs`) and their JavaScript counterparts on `qjs` at N and 2N under `perf stat -e instructions:u`, subtracts each side's empty loop and reports the per-iteration slope.

```sh
cargo build --release -p oxide-core --example workloads
python3 docs/performance/probes/slopes/heap_workloads.py \
    --workloads target/release/examples/workloads --qjs "$CANDIDATE_QJS" --cpu 2 --repeat 5
```

[`slopes/instruction_slopes.py`](slopes/instruction_slopes.py) runs the fixed probes, the heap workloads as JavaScript and a set of feature probes (iteration, spread, classes, generators, typed arrays, collections, string building, sorting) from the root crate's release test binary (its ignored `run_probe_file` test) at N and 2N and reports the per-iteration slopes, nothing subtracted. [`core/examples/loop_probe.rs`](../../../core/examples/loop_probe.rs) runs `empty_loop` and `numeric_local_loop` as hand-built units, for the loop's size and assembly.

```sh
cargo test --release --lib --no-run   # prints the test binary
python3 docs/performance/probes/slopes/instruction_slopes.py --binary "$TEST_BINARY" --cpu 2 --repeat 3
```

`target/release/qjs` (`cargo build --release -p quickjs-oxide-cli --bin qjs`) runs a probe file on the core, so `fixed.py` takes it as an engine. It declares its thread's stack to the parser's stack guard, so it runs under Callgrind unchanged.

`instruction_slopes.py --engine NAME=PATH` measures any engine run as `PATH FILE` (QuickJS, `qjs`). [`slopes/relink.py`](slopes/relink.py) relinks one saved LTO build at several `.text` offsets with identical machine code, the layout method of #119. The root crate's ignored `lowering_census` test (`CENSUS_DIR=DIR cargo test --release --lib lowering_census -- --ignored --nocapture`) lowers every function of every program in a directory on its own and lists the constructs it declines and the functions past the 256-register window.
