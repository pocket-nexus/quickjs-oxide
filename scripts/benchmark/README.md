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

This initial runner measures time, not allocation counts or memory reclamation.
Churn timings include history creation and cannot alone prove a memory bound.
Use storage tests and a separate memory experiment for that acceptance criterion.
All generated programs live in the requested output directory. Results directories
must not already exist, protecting previous evidence from accidental overwrite.

Run admission and workload smoke tests with
`python3 -m unittest discover -s scripts/benchmark -p 'test_*.py'`.
Node, when present, independently checks every generated workload at small sizes;
its absence skips only that check. Repeat a CLI smoke run against Oxide as well.

These tools orchestrate external workloads; they do not vendor benchmark code.
Use Python 3.10+ on a Unix host. Run timing serially, without competing builds
or tests. Compare both sides of one series on the same machine with the same
toolchain and configuration; use a new series for another machine. `run.py` uses process-group timeout
cleanup; Windows process management is not implemented by this runner.

## Prepare binaries with provenance

Commit implementation changes first. Build on the chosen measurement host:

```sh
python3 scripts/benchmark/build.py --jobs 2
```

This builds ordinary and profiling release CLIs in separate target directories.
Use `--plain-only` or `--profile-only` when only one is needed. To build a
selected clean worktree with the current tooling, pass `--repo` and a distinct
target directory, such as `--plain-target /tmp/oxide-base-build`. The receipt
records source and tooling identities separately, snapshots the build/runner
scripts, and rejects a build whose source or tool scripts change while Cargo
runs. It records the release profile and overrides, actual qjs/dependency rustc
flags for crates Cargo recompiles, and paths/hashes of complete build
stdout/stderr logs. The source commit is embedded in profiling
reports. The benchmark
runner verifies matching receipts when present. External engines without a
receipt are identified by binary hash/version output; attach their compiler
and build configuration separately when publishing comparisons.

Ordinary release builds take `lto = "fat"` and `codegen-units = 1` from
`[profile.release]`; comparisons must use the same flags on both sides.

The source identities, build protocol and A/A and A/B design are in
[the performance measurement guide](../../docs/performance/measurement.md).

## External V8 v7 suite

```sh
# A sibling checkout, outside quickjs-oxide; record/fix its commit for repeats.
git clone https://github.com/ahaoboy/js-engine-benchmark.git ../js-engine-benchmark
git -C ../js-engine-benchmark checkout 2034d98fc8c5f8044e186267593f5d5ea5232caf
# Use the project's existing, pinned QuickJS oracle builder if needed.
reference=$(./scripts/quickjs/build-quickjs-oracle.sh)

python3 scripts/benchmark/run.py --suite v8-v7 \
  --source ../js-engine-benchmark \
  --engine oxide="$PWD/target/release/qjs" --engine quickjs="$reference" \
  --repeat 3 --timeout 120 --output target/benchmark-v8-v7
```

`run.py` requires that pinned V8 checkout with no tracked local changes.
Use `--v8-source-commit` only to start an explicitly documented new series.
For two-engine comparisons, `--order abba`, `--order baab`, or
`--order abba-baab` selects complete balanced blocks; repeat must be divisible
by two or four respectively. The default preserves the previous alternating
order. The runner generates bundles under the external checkout's
`dist/quickjs-oxide`, following its `scripts/build.ts` algorithm: inline `load`
calls for the complete suite, or concatenate `base.js`, one suite and the
unchanged runner for isolated cases. It does not change benchmark bodies or
timing policy. The generated complete suite can also be run directly:

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
or mismatched observations before calculating any ratio. The existing runners'
shorter diagnostic schedules remain available; they cannot pass this acceptance
report.

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

Original combined subscore medians are listed separately for comparison with
historical **combined-run** Boa data. They never replace isolated acceptance
results. Reuse unchanged Boa data; this report neither runs Boa nor invents an
aggregate from isolated scores. Compare the final stack directly with its
original baseline, rather than multiplying separately measured stage gains.

### Fixed V8 function and callsite diagnostic

```sh
python3 scripts/benchmark/profile_v8.py \
  --source ../js-engine-benchmark \
  --engine "$PWD/target/profile-feature/release/qjs" \
  --iterations 1 --output /tmp/oxide-v8-fixed-profile
```

`profile_v8.py` uses the same pinned external source and checks its original
Benchmark declarations against the expected names and counts for all eight
subtests. It writes `base.js` plus one unchanged subtest body plus a marked
driver to the requested output directory outside both repositories. The driver
calls each Benchmark's `Setup`, `run` and `TearDown` a fixed number of times;
it does not invoke V8's adaptive `RunSuites`. This is a coverage diagnostic,
**not a V8 Score or a timing comparison**. Use `--case` repeatedly for a subset.

Each fresh process runs `-d --profile-json` with a separate profile output file.
The runner requires the exact completion marker, zero exit, empty script stderr,
one valid compile/VM cost record from a profiling build, and the reported
omission fields. `metadata.json` records pinned source and individual file
hashes, generated workload and driver hashes, binary/build receipt, machine,
and runner identity. `samples.jsonl` and `results.json` retain exact commands,
raw stdout/stderr/profile JSON paths and hashes, completion status, function/PC
and callsite entry counts, and any diagnostic omissions. The CLI's callsite
scope covers selected ordinary driver entry only; it excludes other call and
construct paths, so a zero count is not evidence that JavaScript did no calls.

### Bounded fixed-iteration V8 comparison

For implementation rounds, `iterate_v8.py` compares the eight pinned bodies
separately and as one combined program. It uses **plain release** binaries with
matching build configuration and receipts. The output must be a new directory
outside both repositories:

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
does not prove the host was isolated. The original-score runner above remains
available for a separately planned final release check, not every iteration.

## Pinned QuickJS microbench

```sh
python3 scripts/benchmark/run.py --suite microbench \
  --source target/oracle/quickjs-2026-06-04/tests/microbench.js \
  --engine oxide="$PWD/target/release/qjs" \
  --engine quickjs="$PWD/target/oracle/quickjs-2026-06-04/qjs" \
  --repeat 3 --timeout 120 --output target/benchmark-microbench
```

The default initial matrix is `empty_loop`, `prop_read`, `array_read`,
`func_call`, and `int_arith`. `--case` follows the original function-name prefix
matching. The tool admits ordinary benchmark rows with N and ns/op; specialized
sort output that does not match this contract is retained as incomplete rather
than assigned a score. A shared prefix disables `performance`/`os` clock
selection on both engines, preserving every byte of the original body and using
its existing Date.now fallback. This is necessary because pinned QuickJS adds
`performance.now` even without `--std`, while Oxide currently does not. A dynamic
clock marker is checked on every run; mismatched clocks disqualify the result.
Prepared microbench source stays in an external temporary directory, with its
path/hash and exact prefix/hash recorded in metadata. Its millisecond resolution and minimum-of-many
sampling limit what the resulting ns/op says; these are not individual-operation
latency distributions. No ratio is admitted without the matching clock marker.

Reference loading/saving in JS is not needed. Python records stdout, per-run
ns/op, N, median/min/max/stdev across independent runs and per-case ratios.
Only the shared clock prefix is added; workload bodies are untouched. On engines without `std`/`fs`, the harness's
reference-file operations are no-ops; Python owns result files.

## Cross-engine front-end compile matrix

```sh
python3 scripts/benchmark/compile_workloads.py --output target/compile-corpus
python3 scripts/benchmark/build_compile_probes.py \
  --repo . --output target/compile-probes \
  --quickjs-source target/oracle/quickjs-2026-06-04
python3 scripts/benchmark/compile_matrix.py --metric compile \
  --corpus target/compile-corpus --repeat 5 \
  --engine oxide=target/compile-probes/oxide/target/release/oxide-compile-probe \
  --engine quickjs=target/compile-probes/quickjs/quickjs-compile-probe \
  --engine boa=target/compile-probes/boa/target/release/boa-compile-probe \
  --engine node="$(command -v node)" \
  --output target/compile-matrix
```

`compile_workloads.py` generates deterministic Script-goal sources (syntax-mixed,
functions, expressions) with per-repetition unique names; generated files stay in
the requested output directory. `build_compile_probes.py` builds the QuickJS C
probe against the pinned oracle's `libquickjs.a` and an offline Boa crate outside
the workspace; the Oxide probe reuses `build_compile_probe.py`. `compile_matrix.py`
classifies each engine by its `--version` line, runs one fresh process per sample,
requires exactly one `compile_ns:`/`parse_ns:` line on stdout with empty stderr,
rotates engine order, and never derives a ratio from a partial matrix. `--metric
parse` admits only Boa and V8; Oxide and QuickJS expose no public parse-only entry.
Use `--corpus` with any flat directory of `.js` files, or with a manifest produced
by `compile_workloads.py`. Third-party bundles stay outside the repository.
The runner records raw samples and per-case ratios in its output directory.

## Profiler collection and overhead

```sh
python3 scripts/benchmark/probe.py \
  --plain target/release/qjs --profile target/profile-feature/release/qjs \
  --reference target/oracle/quickjs-2026-06-04/qjs \
  --repeat 11 --output target/profile-experiment
```

This runs an explicitly authored small allocation workload, validates snapshots
and complete scoped trace lifecycles, saves the reference's original output,
and records 100 uninstrumented lifecycle samples. Four instrumentation modes
use the same workload, with mode order rotated between repetitions. Full-process
wall time includes report I/O, so this measurement is workload/host-specific and
not pure interpreter overhead. The reference trace/dump share stdout with the
script, and lifecycle CPU time is kept separate from Oxide wall time.

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
benchmark operation timings are reported separately. These tools do not claim
to isolate parse/compile time without a separate embedding harness.

```sh
python3 -m unittest discover -s scripts/benchmark -p 'test_*.py'
cargo test --locked -p quickjs-oxide-cli --test profiling
cargo test --locked -p quickjs-oxide-cli --test profiling --features profiling
cargo test --locked -p quickjs-oxide --lib --features profiling profiling_
```

Run the broader Rust/QuickJS comparison tests and Test262 independently of
benchmarking. Never revise conformance baselines to turn a performance change
into an apparent pass.

Scaling workloads also cover Array/TypedArray integer reads and writes, repeated interior Array deletion/reinsertion, strict and mapped Arguments construction, and RegExp named groups/indices. Use sizes below 255 for regexp-groups. Mapped arguments use a non-strict Function body explicitly because workload files are modules.

## Replay the fixed-work matrix

`fixed.py` replays a workload manifest.
It checks every source hash before measuring, rotates engine order by default,
and accepts the same balanced `--order` modes as `run.py`. It retains raw
outputs and rejects nonempty stderr. `--workload-dir` relocates existing files;
it never regenerates or silently changes third-party workloads. Reconstruct
missing files using the recipe in the fixed-work report, then verify the hashes.
On macOS, `--darwin-counters` wraps each sample with
`/usr/bin/time -l -o <raw-file>` and records its whole-process retired
instructions, elapsed cycles, maximum resident set size and peak memory
footprint. Missing fields invalidate the counter measurement while preserving
the program's stdout/stderr and the raw time output.

```sh
python3 scripts/benchmark/fixed.py \
  --manifest docs/performance/probes/fixed/manifest.json \
  --workload-dir docs/performance/probes/fixed \
  --engine before=/absolute/baseline/qjs --engine after=/absolute/changed/qjs \
  --repeat 5 --cpu 2 --output target/published-fixed
```

Omitting `--case` covers every manifest entry. Repeated `--case` options are
for step-level experiments only. All times include the whole process; these are
not adaptive harness scores. Preserve build receipts separately and do not run
benchmarks alongside builds, tests or architecture canaries.

All builds use the sole explicit-stack execution core. `build.py` records
`vm_configuration: stack-vm`; no backend-selection feature or legacy build is available.

## Frozen public compilation and original V8 replay

`replay.py` uses the primitive VM baseline receipt's existing 67 source files.
Compile mode includes every entry. Original mode selects the nine entries with
score contracts (the original eight suites and their original combined entry).
Both modes validate source and binary hashes before every run, rotate engine
order, preserve every stdout/stderr and failed round, and reject a comparison
if either engine has a failed round. Ratios always mean **after / before**:
compile elapsed time is lower-is-better; original score is higher-is-better.
Combined scores come only from the original combined workload, never a subset.

Build the public API probe before starting any timed matrix:

```sh
python3 scripts/benchmark/build_compile_probe.py --repo . \
  --output target/candidate-compile
python3 scripts/benchmark/replay.py --mode compile \
  --receipt target/primitive-vm-s07-performance/reproduction/workload-receipt.json \
  --workload-dir target/primitive-vm-s07-performance/reproduction/workloads \
  --engine pr19=/absolute/baseline/compile-probe \
  --engine candidate=target/candidate-compile/target/release/oxide-compile-probe \
  --repeat 10 --output target/candidate-compile-results
python3 scripts/benchmark/replay.py --mode original \
  --receipt target/primitive-vm-s07-performance/reproduction/workload-receipt.json \
  --workload-dir target/primitive-vm-s07-performance/reproduction/workloads \
  --engine pr19=target/primitive-vm-baseline/pr19-qjs \
  --engine candidate=/absolute/candidate/qjs \
  --repeat 5 --timeout 1800 --output target/candidate-original-results
```

The compile probe uses the same `Context::compile_with_filename` boundary as
the frozen S07 probe. Source I/O, Runtime/Context creation and teardown remain
outside its `Instant` interval; it never executes JavaScript. A build with
`profiling` additionally writes phase attribution to stderr and is rejected by
the formal replay harness. Preserve separate build/patch/toolchain receipts
for all binaries. The standalone builder avoids the CLI example dev dependency's
`test-support` feature, validates all registry dependency checksums against the
checkout lockfile, and records toolchain/features/flags. Admission for this frozen S07 experiment requires all 58 fixed entries with
10 rounds in addition to the two replay commands above; this is not the bounded V8 iteration protocol. `--case` subsets only
support directional experiments. Run each matrix serially with builds, tests,
profiling, and CPU/memory sampling stopped.

For a frozen source export, pass `--repo /absolute/source-export` and
`--source-manifest /absolute/source-export.json`; its `files` or `source_files`
map must describe every exported file by SHA-256. The builder validates the
complete inventory and contents both before and after compilation, copies the
receipt alongside the binary metadata, and refuses to use an ancestor Git
checkout's revision or working diff as the export's identity. Keep build output
outside the frozen export. `--profiling` requires a source version containing
the new phase fields; plain probes remain compatible with older baselines.

`--probe`/`--name` build another probe source through the same generated crate
and receipts. The front-end allocation counter
(`scripts/benchmark/probes/compile_alloc_probe.rs`) is built that way and reports
alloc/realloc/dealloc calls and bytes around the same compile window; its
`--version` deliberately matches no matrix magic, so `compile_matrix.py` rejects
it. The probe records its allocation counts alongside the compile window.
