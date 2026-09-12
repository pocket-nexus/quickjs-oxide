# Tachyon comparison adapter

This adapter pins `tachyon-engine/tachyon-engine` at
`2d148e462233c884d0547d4ec0ccc8ccaa183f17`. Its upstream CLI is a placeholder;
this host uses the public compiler/VM API without changing engine code.

```sh
rustup toolchain install 1.95.0 --profile minimal
python3 scripts/benchmark/tachyon/build.py
python3 scripts/benchmark/fixed.py \
  --manifest docs/reports/data-structure-fixed-final.json \
  --engine tachyon=target/tachyon-comparison/source/target/release/examples/qxo-bench \
  --repeat 1 --timeout 30 --cpu 2 \
  --output target/tachyon-comparison/screen
```

The fixed manifest's generated workload files must already exist and match their
recorded SHA256 hashes. Use the existing benchmark preparation documented in
`scripts/benchmark/README.md` to recreate the corpus. Do not replace missing
workloads with smaller or simplified scripts and present them as the same cases.

The adapter reads one script, loads a separate JavaScript `print`/`console.log` prelude (preserving strict directives),
appends an expression retrieving captured output, compiles the workload once, executes it once,
and emits the captured output as UTF-8. The original script body is unchanged.
Compilation, isolate creation, capture, execution, and teardown are all included
in process timing. This differs from a native shell `print`, and is not an
upstream Tachyon CLI result. Console output is deferred until successful
completion; any throw, exhausted budget, compilation error, or output conversion
error exits nonzero. Do not use this adapter for streaming-output benchmarks.
It supplies a real Unix wall clock for `Date`; other host capabilities remain
unavailable. It is a benchmark host, not a general JavaScript shell.

VM resource ceilings are explicit in `main.rs` and `build.json`: 2 GiB managed
heap, 16,384 frames, 4,194,304 registers, 1,048,576 atoms/64 MiB atom bytes,
1,024 loaded modules, and 65,536 globals. These are comparison-host settings,
not Tachyon's checked-in 64 MiB benchmark policy. Execution uses the upstream
MAX/MAX unbounded budget, which selects its optimized unbounded dispatch loop.
Timeouts must be enforced externally.

Use `fixed.py --engine` repeatedly to run all comparators in one rotating matrix.
A result is eligible only when exit status, stdout, stderr, and workload identity
pass admission. Work-count checks in the micro suite do not establish full
semantic correctness: retain separate semantic probes and V8 validations.
Never rank failed or timed-out cases by their short failure duration.

Build artifacts, source checkout, logs, and the binary/toolchain/hash receipt live
under `target/tachyon-comparison/`. The upstream manifest declares Apache-2.0;
its root currently has no standalone LICENSE file. This directory contains our
host adapter only, not copied engine implementation.

## Architecture-focused campaign

Run all commands from the repository root, sequentially. Finish builds before
starting measurements; never overlap a build with timing or profiling. The
recorded campaign used CPU 2 and Linux `perf` / `taskset`.

```sh
python3 scripts/benchmark/tachyon/compare.py --timing-only \
  --engine tachyon=target/tachyon-comparison/source/target/release/examples/qxo-bench \
  --engine oxide=/absolute/path/to/pinned/oxide/qjs \
  --engine quickjs=/absolute/path/to/pinned/quickjs/qjs \
  --output target/tachyon-comparison/run-final
python3 scripts/benchmark/tachyon/architecture_profile.py
python3 scripts/benchmark/tachyon/build_control.py
```

The first command runs semantic probes, screening and five rotating timing
rounds across 58 unchanged workloads. Screening uses a 30-second watchdog with
one 180-second retry for timeouts; the matrix uses an 8 GiB address-space limit.
`--timing-only` skips the broad profile stage. Semantic differences remain in the
journal; this is not Test262 certification.

The focused profiler reads `run-final/metadata.json` and measures eight cases,
with three Tachyon profiles and three counter rounds per engine. It uses the
ordinary ELF, cycles:u period 1,000,003, DWARF 16 KiB and a per-thread 4 MiB
buffer. Reports use leaf attribution. Its symbol cache path is
`target/profile-refresh/symbol-cache`; recreate the matching libc debug cache
when reproducing on another system. Output directories must be new to avoid
mixing campaigns.

`build_control.py` archives PR19 commit
`1cc51bb5fcc5c36912d3197d877219ae513dc4b5`, then builds Oxide with Rust 1.95.0,
thin LTO, one codegen unit, debug=2 and panic=abort. It requires the focused
campaign completion receipt. Use `scripts/benchmark/fixed.py` with the same
manifest, CPU 2, five repetitions, timeout 180, and three `--engine` arguments
named `tachyon`, `oxide`, and `oxide-matched`. Select `empty_loop`, `func_call`,
`int_arith`, `string_length`, and `v8-crypto` with repeated `--case` options;
write results to `target/tachyon-comparison/control`. The matched binary is
`target/tachyon-comparison/oxide-control-source/target/release/qjs`.

For the functional architecture probe, copy `architecture_probe.rs` into the
upstream checkout as `crates/tachyon-vm/examples/qxo-architecture.rs`, build that
example using the same locked release toolchain, and run it. It checks shared
compiled code with isolated globals, `CompiledModule: Send + Sync`, invalid
bytecode rejection, Promise-job quantum resumption after replacing the Future,
and `Value` size. This is a focused behavioral check, not a general safety audit.
The recorded build/output receipt is `architecture-probe.json`.

```sh
python3 scripts/benchmark/tachyon/batch_ablation.py
python3 scripts/benchmark/tachyon/summarize.py
python3 scripts/benchmark/tachyon/report.py
```

The batch ablation saves the ordinary ELF, temporarily changes only the upstream
batch constant from 8 to 1, builds a separate artifact, restores the source and
ordinary ELF, then measures five workloads in alternating order. It records the
exact patch and build hashes. The reducer requires all campaign artifacts and
checks workload/binary hashes, exact outputs, exit status, stderr and profile
loss before producing `summary.json`. The report generator writes the durable
report and JSON under `docs/reports/`, plus a comment body under `target/`.
These reporting commands reproduce the recorded investigation; the raw receipts
and workload files must remain available.

The completed evidence comprises 870 fixed timings, 75 matched-build timings,
50 batch-ablation timings, 24 focused profiles and 72 counter runs. The earlier
broad profile campaign was stopped after the research scope changed; its
incomplete diagnostics are excluded. No RSS or original full V8 harness results
are claimed for this architecture-focused campaign.
