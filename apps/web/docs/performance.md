# Performance

This page gives quickjs-oxide's elapsed time and memory relative to QuickJS
2026-06-04, the engine whose behavior it targets, and how to reproduce the
measurements.

## Results

Measured on 2026-11-14 at
[`df85cbfc`](https://github.com/pocket-nexus/quickjs-oxide/commit/df85cbfc6cbbc52b0a902d2ff81101d89f8c6c9c).
For each program, take each engine's median process time, divide oxide by
QuickJS, then take the equal-weight geometric mean over programs. Below 1×
means less elapsed time. Relative speed is the reciprocal of that ratio.

| Suite | Programs | Time / QuickJS | Relative speed |
| --- | ---: | ---: | ---: |
| V8 benchmark suite v7 | 8 | 0.749× | 1.336× |
| Octane 2.0 | 15 | 0.979× | 1.022× |
| Are-We-Fast-Yet (AWFY) | 14 | 0.954× | 1.048× |
| All programs | 37 | 0.915× | 1.093× |

V8 v7 takes 25.1% less time; all 37 programs take 8.5% less time in the geometric
mean. These are fixed-work process timings, not the suites' adaptive scores.
The V8-only speed ratio is about 1.34×; it does not describe all 37 programs.

Individual results vary: 24 of 37 programs have a lower median time, and 20
clear the per-program A/A-calibrated net-gain gate. The all-program net-gain
gate **failed**. Gameboy takes 2.07× QuickJS's time, CodeLoad 1.93× and AWFY Sieve
1.36×. An aggregate improvement does not establish a gain on every workload.

Both engines ran on one Linux machine with an AMD Ryzen 7 7840HS, pinned to
CPU 2, without competing builds or tests. Oxide used Rust 1.99, optimization
level 3, fat LTO and one code-generation unit; QuickJS used its default `-O2`
build with GCC 16.2.1. Six A/A pairs using the same QuickJS binary preceded six
A/B pairs per program, with ABBA engine order and Linux instruction/cycle
counters. All 444 A/A and 444 A/B samples passed their exit and stdout checks.
The aggregate A/A time ratio was 1.0003×; per-program intervals and the full
37-program table are in the
[Publish measurement record](https://github.com/pocket-nexus/quickjs-oxide/pull/187).

| Suite | Programs |
| --- | --- |
| V8 v7 | Richards, DeltaBlue, Crypto, RayTrace, EarleyBoyer, RegExp, Splay, NavierStokes |
| Octane 2.0 | The eight V8 v7 programs, plus PdfJS, Mandreel, Gameboy, CodeLoad, Box2D, zlib, TypeScript |
| AWFY | DeltaBlue, Richards, Json, CD, Havlak, Bounce, List, Mandelbrot, NBody, Permute, Queens, Sieve, Storage, Towers |

### Memory

Over the same 37 programs, the geometric mean of oxide/QuickJS peak resident
set size (RSS) is **1.542×**, using each engine's median of three runs per
program. All 222 samples succeeded. RSS includes binary pages, the allocator
and other process memory as well as the JavaScript heap.

The collector is a mark-and-sweep collector that runs when the heap reaches a
budget of three times its live bytes by default (at least 1 MiB), so a program's
heap peaks above its live set. QuickJS frees acyclic garbage as soon as it
becomes unreachable. `RuntimeOptions` and the `qjs` flags `--min-heap-mib`,
`--gc-growth` and `--heap-cap-mib` trade collection time for memory
([Errors and limits](./limits.md)).

## How the programs are built

`scripts/benchmark/bench.py suite build` generates one program per benchmark
from checkouts pinned to fixed commits:

| Suite | Source | Commit |
| --- | --- | --- |
| V8 v7 | [js-engine-benchmark](https://github.com/ahaoboy/js-engine-benchmark) | `2034d98f` |
| Octane 2.0 | octane | `570ad1cc` |
| AWFY | are-we-fast-yet | `74306fec` |

V8 and Octane programs concatenate the suite's `base.js` and benchmark files
unchanged, then a driver that resets the random-number generator, calls
`Setup`, calls `run()` a frozen number of times and calls `TearDown`. The run
counts target about a second per program on QuickJS. AWFY bundles the benchmark
modules and calls `innerBenchmarkLoop` with the harness's default inner
iterations, so each benchmark's own result check runs.

No program reads a clock. Each prints a completion line checked on every run.
Time covers startup, compilation, execution and exit. The measurement record
contains the source commits, binary and workload hashes, exact commands,
instruction counts, timing intervals and RSS table. The frozen manifest's
SHA-256 is `b9f4f9e95ef927af49bd1a7d60425a89022c40040e290aa27b64c8da44d30b9d`.
Raw samples and build receipts stay outside version control.

## Reproduce the measurements

Start from a clean, committed tree. Build oxide with a receipt and the pinned,
checksummed reference QuickJS release:

```sh
python3 scripts/benchmark/bench.py build
reference=$(./scripts/quickjs/build-quickjs-oracle.sh)
candidate="$PWD/target/release/qjs"
```

Check out the three suites at their pinned commits and generate the programs.
Use new output directories for each campaign:

```sh
python3 scripts/benchmark/bench.py suite build \
  --v8-source /path/to/js-engine-benchmark --octane-source /path/to/octane \
  --awfy-source /path/to/are-we-fast-yet --output target/suite-matrix
python3 scripts/benchmark/bench.py run --manifest target/suite-matrix/manifest.json \
  --engine quickjs="$reference" --engine control="$reference" \
  --repeat 6 --order abba --cpu 2 --linux-counters --timeout 900 --output target/publish-aa
python3 scripts/benchmark/bench.py run --manifest target/suite-matrix/manifest.json \
  --engine quickjs="$reference" --engine oxide="$candidate" \
  --repeat 6 --order abba --cpu 2 --linux-counters --timeout 900 --output target/publish-ab
python3 scripts/benchmark/bench.py suite table target/publish-ab/results.json \
  --reference quickjs --instructions
python3 scripts/benchmark/bench.py paired --results target/publish-ab/results.json \
  --baseline quickjs --candidate oxide --aa-results target/publish-aa/results.json \
  --aa-baseline quickjs --aa-candidate control --gate net-gain \
  --output target/publish-ab/paired-net-gain.json
```

`suite build` refuses a checkout at another commit. `run` uses fresh processes,
pins them to one CPU, checks stdout and records user-space instructions and
cycles through `perf stat`. Measure serially with no builds, tests or other
benchmarks competing for the machine. The final command exits nonzero when
any program misses its gain gate; preserve and report that result.

### Peak memory

The first engine is the reference for ratios:

```sh
python3 scripts/benchmark/bench.py memory --manifest target/suite-matrix/manifest.json \
  --engine quickjs="$reference" --engine oxide="$candidate" --repeat 3 \
  --output target/publish-memory.json --markdown target/publish-memory.md
```

A small C helper (`vfork`, `execvp`, `wait4`) measures the process's own peak
RSS, taking the median of three runs. The runner evicts each binary from the
page cache before a sample and collects oxide's `--gc-stats --memory-report`
figures separately. A C compiler (`cc`) is required.

### Your own scripts

| Flag | Reports |
| --- | --- |
| `--gc-stats` | Collections, their pause times, live and reserved bytes |
| `--memory-report` | What every arena, the value heap, code and caches hold at exit |
| `--disassemble` | Register code; exits without running the script |

[Optimizations](./optimizations.md) explains the implementation techniques.
The contributor [benchmarking guide](https://github.com/pocket-nexus/quickjs-oxide/blob/main/docs/performance/benchmarking.md)
defines the Sweep, Claim and Publish checks.
