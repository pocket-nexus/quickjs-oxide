# Benchmark tools

`bench.py` is the single entry point; each command's implementation is a
module in [`oxbench/`](oxbench), its tests are in [`tests/`](tests).

```sh
python3 scripts/benchmark/bench.py              # list the commands
python3 scripts/benchmark/bench.py run --help   # one command's options
```

Which command answers which question, and the standard command lines for each
tier of evidence, are in [Benchmarking](../../docs/performance/benchmarking.md).
This page is the reference for each command.

The tools need Python 3.10+ on a Unix host. Run the unit tests with:

```sh
python3 -m unittest discover -s scripts/benchmark -p 'test_*.py'
```

## Common rules

- Every measuring command writes to a new output directory and refuses an
  existing one. `metadata.json` records workload and engine hashes, build
  receipts, machine, OS, source revision and policy; `samples.jsonl` is flushed
  after each sample; raw stdout/stderr stay in separate files.
- Every sample checks the program's exit status and stdout against the
  manifest. A failed, timed-out or wrong-output sample is kept, marked, and
  excludes its case from every ratio.
- Workload and engine bytes are hashed before the run and checked before
  every sample; a change aborts the run.
- Engines are `NAME=PATH`. Two-engine runs accept `--order abba`, `baab` or
  `abba-baab`; `repeat` must be divisible by 2, 2 or 4 respectively. The
  default rotates the engine order per repetition.
- `--cpu N` pins each measured process with `taskset` (Linux).
- Run measurements serially, with no builds, tests or other benchmarks on the
  machine. Results stay in `target/` or outside the repository.

## build

```sh
python3 scripts/benchmark/bench.py build --jobs 2
python3 scripts/benchmark/bench.py build --repo /tmp/oxide-base --plain-target /tmp/oxide-base-target
```

Builds the release CLI (`cargo build --locked --release -p quickjs-oxide-cli
--no-default-features`) from a clean Git worktree and writes
`qjs.build.json` beside the binary. The receipt records the source and tooling
identities, the release profile as written and as overridden by the
environment or Cargo configuration, the `rustc` codegen flags Cargo actually
ran, and paths and hashes of the complete build logs. It snapshots the build
tooling and rejects the build if the source or the tooling changes while Cargo
runs. `--repo` builds another worktree (for example the base commit) with this
checkout's tooling; it needs a distinct `--plain-target`. `--explicit-target`
passes a Cargo target triple.

`run`, `scaling` and `compile matrix` read a matching receipt when one sits
beside the binary and reject a stale one. Release builds take `lto = "fat"` and
`codegen-units = 1` from `[profile.release]`; both sides of a comparison use
the same toolchain and flags.

## suite

```sh
python3 scripts/benchmark/bench.py suite build \
  --v8-source /path/to/js-engine-benchmark --octane-source /path/to/octane \
  --awfy-source /path/to/are-we-fast-yet --output target/suite-matrix
python3 scripts/benchmark/bench.py suite table target/suite-results/results.json \
  --reference quickjs --instructions
```

`suite build` generates one program per item of V8 v7 (8), Octane 2.0 (15) and
AWFY (14) and a manifest for `run`. Each checkout must sit at its pinned commit
(`V8_V7_SOURCE_COMMIT`, `OCTANE_SOURCE_COMMIT`, `AWFY_SOURCE_COMMIT` in
[`oxbench/common.py`](oxbench/common.py) and [`oxbench/suite.py`](oxbench/suite.py)).
Cases are named `v8-v7-<item>`, `octane-<item>` and `awfy-<Name>`.

V8 and Octane programs concatenate `base.js` and the benchmark files unchanged,
then a driver that resets the RNG, calls `Setup`, calls `run()` a frozen number
of times and calls `TearDown` for every Benchmark. The frozen counts make each
program take about a second on QuickJS 2026-06-04 on the reference machine;
Splay's come from instruction counts, its runs being too short to time, and
zlib, Typescript and Mandreel run once. AWFY programs bundle the CommonJS
modules and call `innerBenchmarkLoop` with the harness's default inner
iterations, so each benchmark's own result check runs. No program reads a
clock; each prints one completion line naming its run counts. Changing a count
starts a new baseline. `--pilot N` replaces the frozen counts with N, for
choosing new counts and for the nightly stress run; a pilot manifest is not a
baseline.

`suite table` prints, per workload, the median wall time (ms) and, with
`--instructions`, the median `instructions:u` of each engine, each engine's
time ratio to `--reference` and, with `--instructions`, its instruction ratio
to four places, followed by the geometric means. A workload with any failed
sample shows `fail`.

## fixed-matrix

```sh
python3 scripts/benchmark/bench.py fixed-matrix --check
python3 scripts/benchmark/bench.py fixed-matrix --smoke --output "$OUTPUT/smoke" --oracle node
```

Generates the 20 project-owned diagnostic programs and
[`docs/performance/probes/fixed/manifest.json`](../../docs/performance/probes/fixed/manifest.json),
described in [Diagnostic probes](../../docs/performance/probes/README.md).
`--check` verifies that the committed bytes match the generator; `--smoke` uses
eight loop iterations; `--oracle node` checks the declared outputs with Node.

## sweep

```sh
python3 scripts/benchmark/bench.py sweep --base /tmp/oxide-base-target/release/qjs \
  --candidate target/release/qjs --output /tmp/oxide-sweep
```

The default check of a change. Runs `run --linux-counters` over the 20 fixed
probes and over the eight V8 v7 programs of `--suite` (default
`target/suite-matrix/manifest.json`), base against candidate, `--repeat 4`
(even) in ABBA order on `--cpu 2`, then prints both `suite table`s and
evaluates the instruction gate (each V8 v7 program's candidate/base
`instructions:u` at most 1.02) and the loop-size gate (the candidate's
`vm::interp::run_at` below 16,384 bytes, read with `nm`). Fixed probes above
1.02 are listed without failing. Writes `fixed/`, `v8/` and `summary.md`
under `--output`; exits 1 when a sample or a gate fails. Linux only.

## run

```sh
python3 scripts/benchmark/bench.py run \
  --manifest target/suite-matrix/manifest.json \
  --engine base=/tmp/oxide-base-target/release/qjs --engine candidate="$PWD/target/release/qjs" \
  --repeat 4 --order abba --cpu 2 --linux-counters --output /tmp/oxide-results
```

Runs every manifest entry (or each `--case`) on every engine, one fresh process
per sample, and records whole-process wall time, which includes startup,
compilation, execution and teardown. `--workload-dir` relocates the manifest's
files; their hashes must still match. Nonempty stderr fails a sample. The
default timeout is 180 seconds per sample. `results.json` is the input of
`suite table` and `paired`.

Counter modes, each over the whole process:

- `--linux-counters` wraps each sample in
  `perf stat -x, -e instructions:u,cycles:u,task-clock` and records user-space
  instructions, cycles and task-clock. It needs `perf` and
  `perf_event_paranoid` of 2 or lower.
- `--darwin-counters` wraps each sample in `/usr/bin/time -l` and records
  retired instructions, elapsed cycles, maximum resident set size and peak
  memory footprint. It excludes `--cpu`.
- `--callgrind /path/to/valgrind --callgrind-cache-config cache.json` runs each
  sample under Callgrind instead of timing it. The cache config is a JSON
  object with exactly `I1`, `D1` and `LL`, each
  `[bytes, associativity, line_bytes]`. Cache and branch simulation are on; the
  summary reports per-case median `Ir`, `Dr`, `Dw`, cache-miss and branch
  counters. `--callgrind-instructions` keeps instruction positions and jumps
  for code-generation attribution. These are simulated counts, never speed.

A missing or unparsable counter fails the sample while keeping its output.

## paired

```sh
python3 scripts/benchmark/bench.py paired \
  --results /tmp/oxide-ab/results.json --baseline base --candidate candidate \
  --aa-results /tmp/oxide-aa/results.json --gate net-gain --output /tmp/oxide-ab/paired.json
```

Reads `run` wall-time results and reports, per case, the median of the
**paired** percent gains `100 * (1 - candidate / baseline)` (positive means
faster) with a 95% interval from 10,000 paired bootstrap resamples (seed 79,
empirical percentiles). Admission requires two distinct engines, at least six
complete pairs per case in a declared `abba`, `baab` or `abba-baab` order, the
sample journal in exactly that order, and every sample successful with
unchanged workload bytes; otherwise it rejects the input before forming a
ratio. Callgrind results are rejected.

`--aa-results` names a same-binary A/A run with the same workload bytes, CPU,
machine and settings; each case's noise is the larger absolute endpoint of its
A/A gain interval. `--gate net-gain` passes when every selected case's lower
endpoint exceeds its noise. `--gate noninferior` passes when every lower
endpoint exceeds −1%; A/A noise does not widen this margin, and a slowdown
within it is still reported as negative. A failed gate exits 1. `--case`
selects cases. The JSON keeps every pair's gain, the absolute medians, input
hashes and both runs' metadata; a Markdown table is written beside it. Existing
files are not overwritten. The sample count is fixed before the run; adding
samples until a gate passes invalidates the result.

## memory

```sh
python3 scripts/benchmark/bench.py memory --engine base=/tmp/oxide-base-target/release/qjs \
  --engine candidate=target/release/qjs --markdown /tmp/oxide-memory.md
```

Peak RSS of each `--engine NAME=PATH` over a `run` manifest (default
`target/suite-matrix/manifest.json`; `--case` selects programs). The first
engine is the reference of every ratio. QuickJS takes part only when named,
for example `--engine quickjs=target/oracle/quickjs-2026-06-04/qjs`. Each
sample is one process started by a small C helper (`vfork`, `execvp`,
`wait4`), so `ru_maxrss` is the measured process's own peak. Each engine that
accepts `--gc-stats --memory-report` (`qjs`) runs once more per program for the
collector's and the heap's figures at exit. `--compile-peak` adds the
compile-phase heap peak through massif. The table ends with the geometric mean
of each engine's RSS ratio to the reference.

## scaling

```sh
python3 scripts/benchmark/bench.py scaling \
  --engine before=/absolute/baseline/qjs --engine after=/absolute/changed/qjs \
  --sizes 32 128 512 2048 --operations 32768 --repeat 5 --output target/scaling-comparison
```

Generates project-authored data-structure workloads and records whole-process
wall time per size. Sizes must divide operations for batched workloads; the
runner never rounds the total work. Every program computes an exact result
that the runner checks, and any failed repetition disqualifies its group.

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

Further cases cover Array/TypedArray integer reads and writes, repeated
interior Array deletion and reinsertion, strict and mapped Arguments
construction, and RegExp named groups and indices (sizes below 255 for
`regexp-groups`). Churn timings include history creation; memory bounds come
from storage tests and `memory`.

## compile

```sh
python3 scripts/benchmark/bench.py compile corpus --output target/compile-corpus
python3 scripts/benchmark/bench.py compile probes --repo . --output target/compile-probes \
  --quickjs-source target/oracle/quickjs-2026-06-04
python3 scripts/benchmark/bench.py compile matrix --corpus target/compile-corpus --repeat 5 \
  --engine oxide=target/compile-probes/oxide/target/release/oxide-compile-probe \
  --engine quickjs=target/compile-probes/quickjs/quickjs-compile-probe \
  --output target/compile-matrix
```

- `compile corpus` generates deterministic Script-goal sources (syntax-mixed,
  functions, expressions) with per-repetition unique names and a manifest.
- `compile probe` builds a probe source as a standalone crate against `--repo`,
  by default [`apps/cli/examples/compile_probe.rs`](../../apps/cli/examples/compile_probe.rs),
  which times `Context::compile_with_filename` with source I/O, Runtime/Context
  creation and teardown outside its interval. It copies the checkout's
  lockfile, rejects dependency checksum drift, and records toolchain, features
  and flags. `--probe`/`--name` build another probe source through the same
  crate; [`probes/`](probes) holds `compile_alloc_probe.rs` (allocation calls,
  bytes and peak live bytes in the same window) and `error_alloc_probe.rs` (an
  allocation diagnostic for the public Error channel). For a source export
  without Git metadata, `--source-manifest` names a JSON whose `files` map
  describes every exported file by SHA-256.
- `compile probes` builds the Oxide probe through `compile probe` and a C probe
  against the pinned QuickJS oracle's `libquickjs.a`.
- `compile matrix` classifies each probe by its `--version` line, runs one
  fresh process per sample, requires exactly one `compile_ns:` line and empty
  stderr, rotates probe order, and derives ratios only from a complete matrix.
  `--corpus` takes a `compile corpus` directory or any flat directory of `.js`
  files.

For instruction and allocation counts, `compile_probe_thread` compiles on a
thread with a declared 512 MiB stack, so the parser's stack guard holds under
Valgrind. Its `--version` line is `oxide-compile-probe-thread 1`, which
`compile matrix` does not accept.

```sh
cargo build --release -p quickjs-oxide-cli --example compile_probe_thread
valgrind --tool=callgrind --callgrind-out-file=target/cg.out \
  target/release/examples/compile_probe_thread target/compile-corpus/expressions-524288.js
```

## probes

```sh
python3 scripts/benchmark/bench.py probes --qjs target/release/qjs --v8 "$V8_SOURCE" --output "$OUTPUT/probes"
```

Callgrind at N, 2N and 4N over `work(n)` loops (empty loop, object literal,
`new`, binding, property, array, call and construction probes). Each probe
checks its result and reports per-iteration Ir from the slope, the net value
after subtracting the empty loop, and the 2N − N difference of per-function
self cost. `--v8` adds fixed-iteration V8 v7 bodies (Richards, DeltaBlue,
NavierStokes by default; `--v8-case CASE[=ITERATIONS]` selects) with cache
simulation, and reports their Ir/Dw and per-function self Ir.

## slopes

```sh
cargo test --release --lib --no-run   # prints the test binary
python3 scripts/benchmark/bench.py slopes instructions --binary "$TEST_BINARY" --cpu 2 --repeat 3
cargo build --release -p quickjs-oxide-core --example workloads
python3 scripts/benchmark/bench.py slopes heap \
  --workloads target/release/examples/workloads --qjs target/release/qjs --cpu 2 --repeat 5
```

`slopes instructions` runs the fixed probes, the heap workloads as JavaScript
and feature probes (iteration, spread, classes, generators, typed arrays,
collections, string building, sorting) at N and 2N under
`perf stat -e instructions:u` and reports the per-iteration slope; startup,
compilation and lowering cancel. `--binary` uses the root crate's ignored
`run_probe_file` test; `--engine NAME=PATH` measures any engine run as
`PATH FILE`. `slopes heap` runs the Rust heap workloads
([`core/examples/workloads.rs`](../../core/examples/workloads.rs)) and their
JavaScript counterparts, subtracting each side's empty loop.

## relink

```sh
RUSTFLAGS="-C save-temps --print link-args" cargo build --release \
  -p quickjs-oxide-cli --bin qjs --target-dir /tmp/oxide-relink > /tmp/oxide-relink/build.log 2>&1
python3 scripts/benchmark/bench.py relink /tmp/oxide-relink/build.log /tmp/oxide-relink/qjs 64 128 192
```

Relinks one saved LTO build with `.text` placed at each offset from the
default address, writing `PREFIX-d<delta>` binaries and `PREFIX-relinks.json`
with each variant's `.text` address and SHA-256. The machine code is
identical, so the spread of a `run` over the variants is the layout noise of
that build. A variant whose `.text` bytes differ from the first offset variant
is reported on stderr.
