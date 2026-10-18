# Performance tools

## Data structure scaling

`scaling.py` generates project-authored diagnostic workloads and reuses the
process runner below. It records **whole-process wall time**, including startup,
compilation, setup, validation output and teardown. These are not upstream scores
or compile/link-only measurements.

```sh
python3 scripts/benchmark/scaling.py \
  --engine before=/absolute/baseline/qjs --engine after=/absolute/changed/qjs \
  --sizes 32 128 512 2048 --operations 32768 --repeat 5 \
  --output target/scaling-comparison
```

Use `--case` repeatedly to select workloads. First pilot a workload, then freeze
its operations and sizes for both engines. Sizes must divide operations; the
runner never silently rounds the total work. Samples rotate engine order, retain
stdout/stderr and timeouts, and require an independently computed exact result.
Any failed repetition disqualifies its group. Metadata includes binary hashes,
available verified build receipts, workload files/hashes and generator identity.
Use ordinary builds and the build/provenance procedure below for formal timing.

Only batched workloads require size to divide operations. History, width, key
length and generated-source sizes can vary independently of query counts.

The scaling dimensions differ intentionally:

| Cases | Size changes | Operations controls |
| --- | --- | --- |
| map-int, map-string, set | Entries per collection | Total inserts and membership checks |
| map-churn, set-churn | Prior delete/reinsert history, one live entry | Subsequent membership checks; setup grows with history |
| map-iterate-churn, set-iterate-churn | Prior delete/reinsert history with a paused iterator | New iterators over one live entry; also validate the paused cursor |
| set-intersection | Entries per pair | Total entries across pairs; includes construction |
| prop-write | Object width | Writes to the same existing property |
| prop-delete, array-truncate | Object/array width | Total constructed entries; includes construction and validation |
| scope, constants, module, module-imports | Declarations, names, exports or import/reexport bindings | Unused; each generated program executes once |
| long-key | String length | Repeated lookup of one separately constructed equal key |

Scaling workloads also cover Array/TypedArray integer reads and writes,
repeated interior Array deletion/reinsertion, strict and mapped Arguments
construction, and RegExp named groups/indices. Use sizes below 255 for
`regexp-groups`. Mapped arguments use a non-strict Function body because
workload files are modules.

The runner measures time only. Churn timings include history creation; use
storage tests and a separate memory experiment to establish a memory bound.
All generated programs live in the requested output directory. Results directories
must not already exist, protecting existing evidence from accidental overwrite.

Run admission and workload smoke tests with
`python3 -m unittest discover -s scripts/benchmark -p 'test_*.py'`.
Node, when present, independently checks every generated workload at small sizes;
its absence skips only that check. Repeat a CLI smoke run against Oxide as well.

These tools orchestrate external workloads. Use Python 3.10+ on a Unix host;
`run.py` uses process-group timeout cleanup. Run timing serially, without
competing builds or tests. Compare both sides of one series on the same machine
with the same toolchain and configuration; use a new series for another machine.

## Prepare binaries with provenance

Commit implementation changes first. Build on the chosen measurement host:

```sh
python3 scripts/benchmark/build.py --jobs 2
```

This builds the release CLI with no optional features; `--plain-only` names the
same build explicitly, as PGO requires. To build a
selected clean worktree with the current tooling, pass `--repo` and a distinct
target directory, such as `--plain-target /tmp/oxide-base-build`. The receipt
records source and tooling identities separately, snapshots the build/runner
scripts, and rejects a build whose source or tool scripts change while Cargo
runs. It records the release profile and overrides, actual qjs/dependency rustc
flags for crates Cargo recompiles, and paths/hashes of complete build
stdout/stderr logs. The benchmark runner verifies matching receipts when present. External engines without a
receipt are identified by binary hash/version output; attach their compiler
and build configuration separately when publishing comparisons.

Ordinary release builds take `lto = "fat"` and `codegen-units = 1` from
`[profile.release]`; comparisons must use the same flags on both sides.

The source identities, build protocol and A/A and A/B design are in
[the performance measurement guide](../../docs/performance/measurement.md).

## V8 benchmark suite v7

```sh
# A sibling checkout outside quickjs-oxide, at the commit pinned in run.py.
git clone https://github.com/ahaoboy/js-engine-benchmark.git ../js-engine-benchmark
git -C ../js-engine-benchmark checkout \
  "$(sed -n 's/^V8_V7_SOURCE_COMMIT = "\(.*\)"$/\1/p' scripts/benchmark/run.py)"
# The pinned QuickJS oracle builder supplies the reference engine.
reference=$(./scripts/quickjs/build-quickjs-oracle.sh)

python3 scripts/benchmark/run.py --suite v8-v7 \
  --source ../js-engine-benchmark \
  --engine oxide="$PWD/target/release/qjs" --engine quickjs="$reference" \
  --repeat 3 --timeout 120 --output target/benchmark-v8-v7
```

`run.py` requires that pinned V8 checkout with no tracked local changes.
`--v8-source-commit` starts an explicitly documented new series.
For two-engine comparisons, `--order abba`, `--order baab`, or
`--order abba-baab` selects complete balanced blocks; repeat must be divisible
by two or four respectively. The default alternates engine order per
repetition. The runner generates bundles under the external checkout's
`dist/quickjs-oxide`, following its `scripts/build.ts` algorithm: inline `load`
calls for the complete suite, or concatenate `base.js`, one suite and the
unchanged runner for isolated cases. Benchmark bodies and timing policy stay
unchanged. The generated complete suite can also be run directly:

```sh
./target/release/qjs ../js-engine-benchmark/dist/quickjs-oxide/run.js
```

Default selection is every isolated suite from upstream `run.js`.
Use `--case richards --case deltablue` to select suites, or `--case all` for the
original combined run. Each requested repetition gets a fresh process. Record
both the source commit and generated workload hashes: upstream can change.
The original suite reports scores, not ns/op. Require every expected suite score
and a valid aggregate `Score` even if the engine exits zero: its error callback
can swallow failures. Internal assertions remain the original suite's checks.

### Paired confidence intervals and acceptance

`paired.py` reads `fixed.py` or original V8 `run.py` results. It requires at
least six complete pairs per case in a declared balanced ABBA/BAAB schedule.
It verifies the sample journal order and rejects missing, duplicated, failed,
or mismatched observations before calculating any ratio. The runners' shorter
diagnostic schedules serve diagnostics only and fail this acceptance report.

Freeze a same-binary A/A matrix before comparing candidates. Use the same
workload bytes, CPU affinity, machine and metric for both matrices. On Linux,
both runners accept `--cpu 2` to pin measured processes; a machine snapshot
does not establish that other builds or tests were absent. For the original
V8 matrix, request all eight isolated cases and `--case all` in the same
planned run. The combined program is a separate measurement.

```sh
python3 scripts/benchmark/paired.py \
  --results /tmp/oxide-fixed-ab/results.json \
  --baseline reference --candidate candidate --metric fixed-time \
  --aa-results /tmp/oxide-fixed-aa/results.json \
  --gate net-gain --output /tmp/oxide-fixed-ab/paired.json

python3 scripts/benchmark/paired.py \
  --results /tmp/oxide-original-ab/results.json \
  --baseline reference --candidate candidate --metric original-score \
  --aa-results /tmp/oxide-original-aa/results.json \
  --require-v8-matrix --gate noninferior \
  --output /tmp/oxide-original-ab/paired.json
```

The estimator is the median of **paired** percent gains, with 10,000 paired
bootstrap resamples, seed 79, and an empirical percentile 95% interval. Positive
means better: fixed-work time uses `100 * (1 - candidate / baseline)`; original
Score uses `100 * (candidate / baseline - 1)`. The JSON retains individual gains,
absolute medians, input hashes and both measurement receipts; a Markdown table
is written beside it. Existing files are not overwritten.

The frozen A/A noise for each case and metric is the larger absolute endpoint
of its gain interval. `net-gain` requires the candidate interval's lower endpoint
to exceed that noise. `noninferior` requires a lower endpoint greater than
−1%; A/A noise cannot enlarge this fixed margin. A within-margin slowdown is
still reported with a negative gain. A/A quality is reported separately and
must be considered before using a result. A failed gate returns exit status 1;
invalid evidence is rejected. Do not keep adding samples until a gate passes.

Original combined subscore medians are listed separately from isolated
acceptance results; the report derives no aggregate from isolated scores.
Compare a final change directly with its baseline rather than multiplying
separately measured stage gains.

### Fixed V8 bodies

`profile_v8.py` is the workload generator `iterate_v8.py` and the Callgrind
probes share. It checks the pinned external source's original Benchmark
declarations against the expected names and counts for all eight subtests and
writes `base.js` plus one unchanged subtest body plus a marked driver. The
driver calls each Benchmark's `Setup`, `run` and `TearDown` a fixed number of
times; it does not invoke the suite's adaptive `RunSuites`, so its programs are
**not a V8 Score**.

### Bounded fixed-iteration V8 comparison

`iterate_v8.py` compares the eight pinned bodies
separately and as one combined program. It uses **plain release** binaries with
matching build configuration and receipts. The output must be a new directory
outside both repositories. `--comparison build-technique` compares builds of
the same source, such as a PGO candidate against its matching baseline:

```sh
python3 scripts/benchmark/iterate_v8.py \
  --source ../js-engine-benchmark \
  --baseline /absolute/baseline/release/qjs \
  --candidate /absolute/candidate/release/qjs \
  --darwin-counters --output /tmp/oxide-v8-iteration-1

# Compare another pair against exactly the same generated work and run counts.
python3 scripts/benchmark/iterate_v8.py \
  --source ../js-engine-benchmark \
  --baseline /absolute/other-baseline/release/qjs \
  --candidate /absolute/other-candidate/release/qjs \
  --plan /tmp/oxide-v8-iteration-1/freeze-plan.json \
  --darwin-counters --output /tmp/oxide-v8-iteration-2
```

One invocation gives the baseline one pilot of each isolated suite and the
combined program, then freezes per-suite `run` counts and all nine generated
JS hashes in `freeze-plan.json` before A/A or A/B. `--plan` skips calibration,
regenerates the same inputs and checks the source, driver, tooling and generated
hashes; its saved order and repetition count also govern the replay. For each
original Benchmark, the fixed driver calls `Setup` once, does zero warmup calls
by default (`--warmup` changes that fixed count), invokes `run` a frozen number
of times, and calls `TearDown` once. The original body and its internal checks
are untouched. Each process loads the pinned `base.js` once; that version seeds
deterministic `Math.random` there and has no separate `ResetRNG` function. The
combined program loads the eight bodies in the original `run.js` order.

The default requests four repetitions per engine in each of same-binary A/A
and baseline/candidate A/B, in balanced ABBA-BAAB blocks. If the pilot predicts
that even one `run` per Benchmark cannot fit this schedule, it falls back to
two repetitions per engine in ABBA order. Pilot, freeze and both comparisons
share a **600-second sampling deadline**; final result serialization and process
cleanup can add a little time after that deadline. A slower machine or workload can still
finish incomplete; timeout, bad output, changed bytes or missing samples leave
`summary.aggregate` null. Raw outputs and statuses remain in the new directory.
The summary's eight-suite geometric mean and combined whole-process speed ratio
are fixed-work diagnostics, **not the original adaptive V8-v7 Score**. Do not
admit a small change within the observed A/A spread; annotate known concurrent
work or other timing interference in the result receipt. The machine snapshot
does not prove the host was isolated. Use the original-score runner above for
a separately planned final check.

## Pinned QuickJS microbench

```sh
python3 scripts/benchmark/run.py --suite microbench \
  --source target/oracle/quickjs-2026-06-04/tests/microbench.js \
  --engine oxide="$PWD/target/release/qjs" \
  --engine quickjs="$PWD/target/oracle/quickjs-2026-06-04/qjs" \
  --repeat 3 --timeout 120 --output target/benchmark-microbench
```

The default matrix is `empty_loop`, `prop_read`, `array_read`,
`func_call`, and `int_arith`. `--case` follows the original function-name prefix
matching. The tool admits ordinary benchmark rows with N and ns/op; specialized
sort output that does not match this contract is retained as incomplete rather
than assigned a score. A shared prefix disables `performance`/`os` clock
selection on both engines, preserving every byte of the original body and using
its existing Date.now fallback, so both engines use the same clock. A dynamic
clock marker is checked on every run; mismatched clocks disqualify the result.
Prepared microbench source stays in an external temporary directory, with its
path/hash and exact prefix/hash recorded in metadata. Its millisecond resolution and minimum-of-many
sampling limit what the resulting ns/op says; these are not individual-operation
latency distributions. No ratio is admitted without the matching clock marker.

Python records stdout, per-run ns/op, N, median/min/max/stdev across
independent runs and per-case ratios, and owns all result files. On engines
without `std`/`fs`, the harness's reference-file operations are no-ops.

## Front-end compile matrix

```sh
python3 scripts/benchmark/compile_workloads.py --output target/compile-corpus
python3 scripts/benchmark/build_compile_probes.py \
  --repo . --output target/compile-probes \
  --quickjs-source target/oracle/quickjs-2026-06-04
python3 scripts/benchmark/compile_matrix.py \
  --corpus target/compile-corpus --repeat 5 \
  --engine oxide=target/compile-probes/oxide/target/release/oxide-compile-probe \
  --engine quickjs=target/compile-probes/quickjs/quickjs-compile-probe \
  --output target/compile-matrix
```

`compile_workloads.py` generates deterministic Script-goal sources (syntax-mixed,
functions, expressions) with per-repetition unique names; generated files stay in
the requested output directory. `build_compile_probes.py` builds the QuickJS C
probe against the pinned oracle's `libquickjs.a`; the Oxide probe reuses
`build_compile_probe.py`. `compile_matrix.py` classifies each probe by its
`--version` line, runs one fresh process per sample, requires exactly one
`compile_ns:` line on stdout with empty stderr, rotates probe order, and derives
ratios only from a complete matrix.
Use `--corpus` with any flat directory of `.js` files, or with a manifest produced
by `compile_workloads.py`. Third-party bundles stay outside the repository.
The runner records raw samples and per-case ratios in its output directory.

## Output and validation

Every invocation requires a new output directory, preserving prior evidence.
`metadata.json` captures workload hashes, engine hashes/build receipts, hardware,
OS, source revision, clock and policy. `samples.jsonl` is flushed after each
sample so interrupted work retains evidence. `results.json` and `report.md`
contain the final matrix, raw sample references and summaries. Raw stdout/stderr
remain separate files. Nonzero exit, timeout, incomplete/unsupported output and
successful samples are distinct. The runner returns nonzero if any sample is
unsuccessful. A case qualifies for a cross-engine ratio only if every requested
repetition succeeds on both engines; no overall score is inferred from a subset.

The built-in harness warm-up/calibration is unchanged and recorded as such.
Process wall times include startup, parse/compile, execution and teardown;
benchmark operation timings are reported separately. Compile-only timing uses
the compile probes below.

```sh
python3 -m unittest discover -s scripts/benchmark -p 'test_*.py'
```

Run the broader Rust/QuickJS comparison tests and Test262 independently of
benchmarking. Conformance baselines change only for conformance reasons.

## Replay the fixed-work matrix

`fixed.py` replays a workload manifest, such as the one generated by
[`build_fixed_matrix.py`](../../docs/performance/probes/README.md).
It checks every source hash before measuring, rotates engine order by default,
and accepts the same balanced `--order` modes as `run.py`. It retains raw
outputs and rejects nonempty stderr. `--workload-dir` relocates existing files;
the manifest hashes must still match.

```sh
python3 scripts/benchmark/fixed.py \
  --manifest docs/performance/probes/fixed/manifest.json \
  --workload-dir docs/performance/probes/fixed \
  --engine before=/absolute/baseline/qjs --engine after=/absolute/changed/qjs \
  --repeat 5 --cpu 2 --output target/fixed-results
```

Omitting `--case` covers every manifest entry; repeated `--case` options select
a subset. All times include the whole process; these are not adaptive harness
scores. Preserve build receipts separately and run benchmarks without
concurrent builds or tests.

On macOS, `--darwin-counters` wraps each sample with
`/usr/bin/time -l -o <raw-file>` and records its whole-process retired
instructions, elapsed cycles, maximum resident set size and peak memory
footprint. Missing fields invalidate the counter measurement while preserving
the program's stdout/stderr and the raw time output.

On Linux, `--linux-counters` wraps each sample with
`perf stat -x, -e instructions:u,cycles:u,task-clock -o <raw-file>` and
records user-space instructions, cycles and task-clock milliseconds for the
whole process. It needs `perf` and a `perf_event_paranoid` setting that allows
counting one's own processes (2 or lower).

`--callgrind /path/to/valgrind --callgrind-cache-config cache.json` runs each
sample under Callgrind (`callgrind.py`) instead of timing it. The cache config
is a JSON object with exactly `I1`, `D1` and `LL`, each
`[bytes, associativity, line_bytes]`. Cache and branch simulation are enabled;
the summary reports per-case median `Ir`, `Dr`, `Dw`, cache-miss and branch
counters for the whole process. `--callgrind-instructions` also keeps
instruction positions and jumps for code-generation attribution. These are
simulated counters, not native timings.

## Fixed-work V8 v7, Octane and AWFY matrix

`suite_matrix.py build` generates one program per item of V8 v7 (8), Octane 2.0
(15) and AWFY (14) from pinned upstream checkouts (js-engine-benchmark
`2034d98f`, octane `570ad1cc`, are-we-fast-yet `74306fec`) and writes a
`fixed.py` manifest. V8 and Octane programs concatenate `base.js` and the
benchmark files unchanged, then a driver that resets the RNG, calls `Setup`,
calls `run()` a frozen number of times and calls `TearDown` for every
Benchmark; AWFY programs bundle the CommonJS modules and call
`innerBenchmarkLoop` with the harness's default inner iterations, so the
benchmark's own result check runs. No program reads a clock, and each prints
one completion line naming its run counts. `--pilot N` replaces the frozen
counts for choosing new ones; a pilot manifest is not a baseline.

```sh
python3 scripts/benchmark/suite_matrix.py build \
  --v8-source /path/to/js-engine-benchmark --octane-source /path/to/octane \
  --awfy-source /path/to/are-we-fast-yet --output target/suite-matrix
python3 scripts/benchmark/fixed.py --manifest target/suite-matrix/manifest.json \
  --engine quickjs=/path/to/quickjs/qjs --engine oxide=/path/to/qjs \
  --repeat 5 --cpu 2 --linux-counters --timeout 900 --output target/suite-results
python3 scripts/benchmark/suite_matrix.py table target/suite-results/results.json \
  --reference quickjs --instructions
```

`table` prints the median wall time and instructions per workload and engine,
each engine's time ratio to the reference, and their geometric mean; a
workload with any failed sample shows `fail`.

## Profile-guided optimization

```sh
python3 scripts/benchmark/pgo_verified.py --output /absolute/outside/pgo-run
```

`pgo_verified.py` requires a clean source worktree and an output directory
outside the repositories. Through `build.py --optimization`, it builds an
instrumented CLI, trains it on every scaling workload, merges the raw profiles
with the `llvm-profdata` matching rustc's LLVM version, writes
`training.json`, then builds the profile-use candidate and a matching
uninstrumented baseline for the same target. `engines.json` names both
binaries. V8 v7 stays out of training so `iterate_v8.py --comparison
build-technique` can evaluate the candidate independently. `build.py
--optimization profile-use` accepts only a training receipt that
`pgo_validation.py` verifies against the source and every recorded artifact.

`pgo.py` is a quicker unverified builder: it builds into `target/pgo-generate`,
trains on scaling workloads (plus V8 v7 with `--v8-source`), merges profiles
in `target/pgo-profiles`, and writes the optimized CLI to `target/pgo-use`.

## Compile probes and frozen-workload replay

`build_compile_probe.py` builds a probe source as a standalone crate against
`--repo`, by default `apps/cli/examples/compile_probe.rs`. That probe times
`Context::compile_with_filename`; source I/O, Runtime/Context creation and
teardown stay outside its `Instant` interval, and it never executes JavaScript.
The builder copies the checkout's lockfile, rejects dependency
checksum drift, and records toolchain, features and flags.

`replay.py` replays a receipt whose `workloads` entries give `case`, `path`,
`sha256` and `expected`. Compile mode uses every entry and requires exactly one
`compile_ns:` line. Original mode selects entries whose `expected` is a list
of V8 score names. Both modes validate source and binary hashes before every
run, rotate engine order, preserve every stdout/stderr and failed round, and
reject a comparison if either engine has a failed round. Ratios mean
**after / before**: compile time is lower-is-better; original score is
higher-is-better. Default repetitions are 10 for compile and 5 for original.

```sh
python3 scripts/benchmark/build_compile_probe.py --repo . \
  --output target/candidate-compile
python3 scripts/benchmark/replay.py --mode compile \
  --receipt /absolute/workloads/receipt.json \
  --workload-dir /absolute/workloads \
  --engine baseline=/absolute/baseline/oxide-compile-probe \
  --engine candidate=target/candidate-compile/target/release/oxide-compile-probe \
  --output target/candidate-compile-results
python3 scripts/benchmark/replay.py --mode original \
  --receipt /absolute/workloads/receipt.json \
  --workload-dir /absolute/workloads \
  --engine baseline=/absolute/baseline/qjs \
  --engine candidate=/absolute/candidate/qjs \
  --output target/candidate-original-results
```

Run each matrix serially with builds, tests, profiling, and CPU/memory sampling
stopped. `--case` subsets support directional comparisons.

For a source export without Git metadata, pass `--repo /absolute/source-export`
and `--source-manifest /absolute/source-export.json`; its `files` or
`source_files` map must describe every exported file by SHA-256. The builder
validates the complete inventory and contents before and after compilation,
copies the manifest alongside the binary metadata, and requires build output
outside the export.

`--probe`/`--name` build another probe source through the same generated crate
and receipts. `scripts/benchmark/probes/` holds:

- `compile_alloc_probe.rs`: counts alloc/realloc/dealloc calls, bytes and peak
  live bytes around the same compile window. Its `--version` matches no matrix
  magic, so `compile_matrix.py` rejects it.
- `error_alloc_probe.rs`: an allocation diagnostic for the public Error channel.
