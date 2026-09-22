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
Use Python 3.10+ on a Unix host. Run timing and heavier validation on PocketLab,
serially, without competing builds or tests. `run.py` uses process-group timeout
cleanup; Windows process management is not implemented by this runner.

## Prepare binaries with provenance

Build a clean checkout on the measurement host:

```sh
python3 scripts/benchmark/build.py --jobs 2
```

For uncommitted work, export the complete source (including new files), record
each file's SHA-256 in a `files` or `source_files` map, and build that frozen
export instead. Keep the manifest and build outputs outside the export:

```sh
python3 scripts/benchmark/build.py --repo /absolute/source-export \
  --source-manifest /absolute/source-export.json --jobs 2 \
  --plain-target /absolute/build/plain --profile-target /absolute/build/profiling
```

Both builders validate the complete inventory and file contents before and
after compilation. An export cannot inherit its parent checkout's Git identity.
The CLI builder embeds `frozen:<manifest-sha256>` and copies the exact source
manifest to `qjs.source.json`. Its receipt also records Cargo profile/target
environment overrides. `--mode plain` or `--mode profiling` builds only that mode;
the default is both. Cargo's reported executable determines the actual binary
path, including a configured target triple. All selected builds must finish
with unchanged source identity before any new receipts are published.

This builds ordinary and profiling release CLIs in separate target directories,
embeds the source commit in profiling reports, and writes `qjs.build.json`
receipts with compiler versions, command, flags and binary hash. The benchmark
runner verifies matching receipts when present. External engines without a
receipt are identified by binary hash/version output; attach their compiler
and build configuration separately when publishing comparisons.

### Internal QuickOp publication experiment

The default build (M0) does not store or allocate QuickOp projections. B1b's M1
experiment eagerly builds authenticated words at publication and still executes
canonical instructions. It is selected by the internal Rust cfg
`oxide_quick_projection`, not a public runtime mode or Cargo feature:

```sh
RUSTFLAGS='--cfg oxide_quick_projection' python3 scripts/benchmark/build.py \
  --repo /absolute/source-export --source-manifest /absolute/source-export.json \
  --mode plain --plain-target /absolute/build/m1 --jobs 2
```

Unset `CARGO_ENCODED_RUSTFLAGS` before this command: Cargo gives it precedence
over `RUSTFLAGS`. Build M0 from the exact same export without the cfg in a
separate directory, with identical release LTO/codegen settings. Preserve both
receipts; M1 remains an experiment until compile, memory and execution gates pass.
There is no QuickOp dispatch in B1b.

With `profiling`, `quick_projection` is a nested compilation phase. The additive
`quick_projection_counts` map counts successful **builds**, canonical PC tag
distribution and cumulative word capacities; it is not live memory or executed
opcodes. `maximum_observed_ir_capacity_bytes` is the largest per-function word
buffer, not a process or compiler peak. Memory snapshots separately deduplicate
`bytecode_quick_words` by shared buffer identity and report a labelled
`bytecode_quick_controls` estimate (Vec header plus two Rc counters). Allocator
overhead remains unknown. Existing arena/executable categories include the
actual inline field sizes; never add those fields again. CLI metadata labels
the experiment as `eager-experiment-canonical-execution` or `disabled`.

### Direct-store diagnostic inputs

`direct_store.py` freezes project-authored local/argument consume/keep workloads
and boundary/fusion protection cases, including exact stdout and source hashes:

```sh
python3 scripts/benchmark/direct_store.py --iterations 100000 \
  --output target/direct-store-inputs
python3 scripts/benchmark/fixed.py \
  --manifest target/direct-store-inputs/manifest.json \
  --engine before=/absolute/before/qjs --engine after=/absolute/after/qjs \
  --repeat 10 --cpu 2 --output target/direct-store-results
```

First confirm the manifest's minimum profiling event counts with small inputs
in a separate instrumented run. Consume cases use loop-local `var` initializers:
ordinary assignment statements currently compile to keep stores followed by
Drop, so their source spelling does not prove consume coverage. Pilot on the
baseline, freeze adequate workload sizes, and run A/A before A/B. These targeted
diagnostics do not replace the historical full matrices or stage prerequisites.

## Profile-guided optimization

`pgo.py` builds a profile-guided CLI in three phases: an instrumented build, a
training run, and an optimized build. It needs the rustup `llvm-tools`
component for `llvm-profdata`:

```sh
rustup component add llvm-tools
python3 scripts/benchmark/pgo.py --jobs 16 --v8-source ../js-engine-benchmark
```

By default every `scaling.py` case is trained at sizes 64 and 128, and the
external v8-v7 suite is added when `--v8-source` points at an
`js-engine-benchmark` checkout outside this repository. Training failures only
shrink coverage: raw profiles are kept and merged anyway. The optimized binary
is written under `--use-target` with a `qjs.build.json` receipt recording the
merged profile hash, training load and compiler flags. `--skip-training`
rebuilds from existing raw profiles. Ordinary release builds also take
`lto = "fat"` and `codegen-units = 1` from `[profile.release]`; comparisons
must use the same flags on both sides.

Protocol for comparisons during staged performance work (revised 2026-09-22):

- Comparisons use the release profile as shipped: **fat LTO with
  `codegen-units = 1`** (the `[profile.release]` defaults), identical flags on
  both sides, no PGO and no `profiling` feature. Baselines must be rebuilt with
  these flags before comparing. Record the source revision, toolchain, target,
  effective flags, and binary/workload hashes for both sides; a historical
  LTO-off binary is not a valid denominator for a current stage comparison.
- Rationale for the revision: the earlier LTO-off/CGU=16 protocol was meant to
  keep regressions visible, but measured practice showed CGU partitioning
  itself injects ±5–10% layout noise (cross-module inlining flips on unrelated
  edits), and it diverges from the shipped configuration. Stage A records up to
  §8.12 used the old protocol; those series stay valid against their own
  LTO-off baselines and must not be mixed with LTO-on numbers.
- Each stage is compared twice: against the previous stage and against the
  saved baseline. Identify both baseline source revisions explicitly. If the
  post-E saved baseline includes stage A, also retain a separate comparison to
  pre-A `85afd564`, rebuilt with the same fat LTO/CGU1/no-PGO flags, to track
  unresolved stage A regressions. Improvements over post-E do not by themselves
  close those regressions.
- Per-stage PGO retraining is **not** required; at the close of each major stage
  a full LTO+PGO check (both sides independently retrained with the same training
  workloads) is recommended. Report it separately; PGO gains cannot offset
  regressions in the ordinary no-PGO release gate.
- Cross-protocol comparisons are accepted for cumulative, user-facing deltas;
  label the build protocol of both sides. They must not be used for stage
  acceptance or to attribute performance changes to a code change.

## External V8 v7 suite

```sh
# A sibling checkout, outside quickjs-oxide; record/fix its commit for repeats.
git clone https://github.com/ahaoboy/js-engine-benchmark.git ../js-engine-benchmark
# Use the project's existing, pinned QuickJS oracle builder if needed.
reference=$(./scripts/quickjs/build-quickjs-oracle.sh)

python3 scripts/benchmark/run.py --suite v8-v7 \
  --source ../js-engine-benchmark \
  --engine oxide="$PWD/target/release/qjs" --engine quickjs="$reference" \
  --repeat 3 --timeout 120 --output target/benchmark-v8-v7
```

The runner generates bundles under the external checkout's
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

`fixed.py` replays the workload manifest in the final data-structure report.
It checks every source hash before measuring, rotates engine order, retains raw
outputs and rejects nonempty stderr. `--workload-dir` relocates existing files;
it never regenerates or silently changes third-party workloads. Reconstruct
missing files using the recipe in the fixed-work report, then verify the hashes.

```sh
python3 scripts/benchmark/fixed.py \
  --manifest docs/reports/data-structure-fixed-final.json \
  --engine before=/absolute/baseline/qjs --engine after=/absolute/changed/qjs \
  --repeat 5 --cpu 2 --output target/published-fixed
```

Omitting `--case` covers all 58 manifest entries. Repeated `--case` options are
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
checkout lockfile, and records toolchain/features/flags. Formal stages require all 58 fixed entries with
10 rounds in addition to the two replay commands above; `--case` subsets only
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

## First execution after one compile

`first_execution_probe.rs` creates a fresh Runtime and Context, installs qjs
helpers, compiles the frozen Script once, then measures the first
`Context::execute` directly with `Instant`. Its measured interval includes
first-use execution projections, nested eval compilation, host calls/output
and execution-time GC. Source reading, Runtime/Context/helper setup, outer
compile/publication, pending jobs, JSON output and teardown are outside that
interval. This is not a cold-machine measurement: compilation has already
touched code/data. Pending jobs are drained separately and any sample with a
job is rejected, so use synchronous Script workloads without module loading or
script arguments.

Build every mode serially from its frozen tree using the same compiler and
release flags (fat LTO/CGU1, no PGO for the ordinary cohort). The shared builder
accepts `--probe first-execution`; all existing source-manifest, dependency
checksum and binary receipt checks apply. For example:

```sh
CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
RUSTFLAGS='--cfg oxide_quick_projection' \
python3 scripts/benchmark/build_compile_probe.py --probe first-execution \
  --repo /absolute/m1-frozen-source \
  --source-manifest /absolute/m1-source-manifest.json \
  --output /absolute/m1-first-probe

python3 scripts/benchmark/first_execution.py prepare \
  --manifest /absolute/quick-resource-inputs/manifest.json \
  --engine M0=/absolute/m0-first-probe/target/release/oxide-first-execution-probe \
  --engine M1=/absolute/m1-first-probe/target/release/oxide-first-execution-probe \
  --output target/first-execution-m0-m1
python3 scripts/benchmark/first_execution.py run \
  --output target/first-execution-m0-m1
```

The runner also accepts the existing `fixed.py` manifest shape, optional
`--workload-dir`/`--case`, and `--engine M2=...`. It requires adjacent build
receipts, an identical probe source hash, explicit `CARGO_PROFILE_RELEASE_LTO=fat`
and `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1`, a release command and no instrumented
features or PGO flags. Compiler, target, wrappers and release flag provenance
must match across modes; only `oxide_quick_projection` and
`oxide_quick_dispatch` cfg differences are allowed. The builder records the
compilation environment whitelist, visible Cargo configuration identities and
relevant profile/target settings, and compiler/wrapper file hashes. The runner
rejects unresolved Cargo `[env]` compiler overrides and `--config` recipes.
Mode labels describe supplied binaries; inspect their source and
flags receipts to establish which dispatch implementation they contain.
Preparation freezes the protocol before launching any candidate: CPU 2,
10 fresh-process samples per mode/case, rotating mode order, workload/oracle,
runner/helper, source and binary hashes. A/A can supply the same binary as two
mode labels in a separate output directory.

Each raw sample preserves stdout, stderr, exit status, process wall time and a
separate JSON timing file. Exact stdout, empty stderr, successful exit, one
compile/execute, no pending jobs and nonzero integer durations are mandatory.
Failed samples are retained and make the corresponding comparison ineligible;
they never become zero. Only `first_execute_ns` enters the timing comparison.
The same process's `compile_ns` is diagnostic, and `process_wall_ns` stays
separate. Continue to run dedicated compile, whole-process cold and independent
RSS cohorts; do not infer first execution by subtracting compile from cold or
use this probe to skip the other cost gates.
