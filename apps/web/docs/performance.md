# Performance

This page gives quickjs-oxide's speed and memory relative to QuickJS 2026-06-04, the engine whose behavior it targets, and the commands that reproduce the measurements.

## Results

The benchmark matrix has 37 fixed-work programs from three suites. Each figure is the geometric mean, over the suite's programs, of quickjs-oxide's run time divided by QuickJS 2026-06-04's. Below 1× quickjs-oxide is faster; above 1× it is slower.

| Suite | Programs | Time relative to QuickJS |
| --- | ---: | ---: |
| V8 benchmark suite v7 | 8 | 0.94× |
| Octane 2.0 | 15 | 1.15× |
| Are-We-Fast-Yet (AWFY) | 14 | 0.99× |

Both engines are release builds measured on the same Linux machine.

| Suite | Programs |
| --- | --- |
| V8 v7 | Richards, DeltaBlue, Crypto, RayTrace, EarleyBoyer, RegExp, Splay, NavierStokes |
| Octane 2.0 | The eight V8 v7 programs, plus PdfJS, Mandreel, Gameboy, CodeLoad, Box2D, zlib, TypeScript |
| AWFY | DeltaBlue, Richards, Json, CD, Havlak, Bounce, List, Mandelbrot, NBody, Permute, Queens, Sieve, Storage, Towers |

### Memory

Over the same 37 programs, the geometric mean of quickjs-oxide's peak resident set size (RSS) is 1.59× QuickJS's.

The collector is a mark-and-sweep collector that runs when the heap reaches a budget of three times its live bytes by default (at least 1 MiB), so a program's heap peaks above its live set. QuickJS frees acyclic garbage as soon as it becomes unreachable and peaks close to its live set. `RuntimeOptions` and the `qjs` flags `--min-heap-mib`, `--gc-growth` and `--heap-cap-mib` trade collection time for memory ([Errors and limits](./limits.md)).

## How the programs are built

`scripts/benchmark/bench.py suite build` generates one program per benchmark from checkouts pinned to fixed commits:

| Suite | Source | Commit |
| --- | --- | --- |
| V8 v7 | [js-engine-benchmark](https://github.com/ahaoboy/js-engine-benchmark) | `2034d98f` |
| Octane 2.0 | octane | `570ad1cc` |
| AWFY | are-we-fast-yet | `74306fec` |

V8 and Octane programs concatenate the suite's `base.js` and the benchmark's files unchanged, then a driver that resets the random-number generator, calls `Setup`, calls `run()` a frozen number of times and calls `TearDown`. The run counts are chosen so that each program takes about a second on QuickJS 2026-06-04. AWFY programs bundle the benchmark's modules and call `innerBenchmarkLoop` with the harness's default inner iterations, so each benchmark's own result check runs.

No program reads a clock. Each prints one completion line, which the runner checks on every run. The measured time is the whole process: startup, compilation, execution and exit. These are fixed-work timings, not the suites' adaptive scores.

## Reproduce the measurements

Build quickjs-oxide and the reference QuickJS. The repository builds the pinned QuickJS 2026-06-04 release from its checksummed archive and prints the path of its `qjs`:

```sh
cargo build --locked --release -p quickjs-oxide-cli
reference=$(./scripts/quickjs/build-quickjs-oracle.sh)
```

The release profile uses `lto = "fat"` and `codegen-units = 1`. Compare builds made with the same toolchain and configuration.

Check out the three suites at their pinned commits, generate the programs, run them, and print the table:

```sh
python3 scripts/benchmark/bench.py suite build \
  --v8-source /path/to/js-engine-benchmark --octane-source /path/to/octane \
  --awfy-source /path/to/are-we-fast-yet --output target/suite-matrix
python3 scripts/benchmark/bench.py run --manifest target/suite-matrix/manifest.json \
  --engine quickjs="$reference" --engine oxide="$PWD/target/release/qjs" \
  --repeat 5 --cpu 2 --linux-counters --timeout 900 --output target/suite-results
python3 scripts/benchmark/bench.py suite table target/suite-results/results.json \
  --reference quickjs --instructions
```

`suite build` refuses a checkout at any other commit. `run` runs each sample in a fresh process, alternates the engine order, pins the processes to one CPU with `--cpu`, and with `--linux-counters` records user-space instructions and cycles through `perf stat`. It needs a new output directory every time. `table` prints each program's median wall time and instruction count per engine, each engine's time and instruction ratios to the reference, and the geometric means. Run the matrix with no builds or tests competing for the machine.

### Peak memory

`scripts/benchmark/bench.py memory` measures peak RSS of each named engine over the same programs, by default those of `target/suite-matrix/manifest.json`; the first engine is the reference of the ratios:

```sh
python3 scripts/benchmark/bench.py memory --engine quickjs="$reference" --engine oxide="$PWD/target/release/qjs" \
  --markdown target/memory-table.md
```

It starts each sample through a small C helper (`vfork`, `execvp`, `wait4`), so `ru_maxrss` is the measured process's own peak, and takes the median of 3 runs. Before measuring it evicts every binary from the page cache. It runs `qjs --gc-stats --memory-report` once per program for the collector's and the heap's figures, and prints the geometric mean of the ratio. It needs a C compiler (`cc`).

### Your own scripts

`qjs` reports what a script costs inside the engine:

| Flag | Reports |
| --- | --- |
| `--gc-stats` | Collections, their pause times, live and reserved bytes |
| `--memory-report` | What every arena, the value heap, the code and the caches hold at exit |
| `--disassemble` | The register code of every function |
| `--ic-stats` | Inline-cache hits and misses, in a build with the `ic-stats` feature |

## Where to go next

- [Optimizations](./optimizations.md): inline caches, NaN-boxing, shapes and the collector.
- [Architecture](./architecture.md): the crates and how a script runs.
- [Errors and limits](./limits.md): heap caps and the collector's settings.
- [`scripts/benchmark/README.md`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/scripts/benchmark/README.md): every benchmark tool, including the paired A/B comparison and the compile-time matrix.
