# Profiling and diagnostics

`qjs` prints the collector's statistics, the memory report, the inline-cache report and the register code of a script. The reports go to stderr when the script ends, so program stdout is unchanged. [Measurement guidance](performance/measurement.md) describes how these diagnostics accompany ordinary release timing; [Execution](performance/execution.md) and [the heap](heap.md) describe what they count.

## Run

```sh
cargo build --locked --release -p quickjs-oxide-cli
./target/release/qjs --gc-stats workload.js
./target/release/qjs --memory-report workload.js
./target/release/qjs --disassemble workload.js
cargo build --locked --release -p quickjs-oxide-cli --features ic-stats --target-dir target/ic-stats
./target/ic-stats/release/qjs --ic-stats workload.js
```

| Option | Report |
| --- | --- |
| `--gc-stats` | One line: collections, total pause, the pauses' p50, p99 and maximum, live and reserved KiB |
| `--memory-report` | Per arena (objects, shapes, strings, symbols, BigInts, cells, functions, bytecode, internals, values): live records, capacity, used and reserved bytes; free value ranges by size class and free runs; rope bytes, atoms and the tables beside the arenas; the heap's reserved bytes, collection threshold, budget and cap; code units, instructions, positions, source and RegExp programs; the register file's high-water mark and capacity; inline-cache entries; the collector's totals |
| `--ic-stats` | Per cache kind (get, set, global, construct): monomorphic and polymorphic hits, fills, unfilled lookups and hit rate; sites by state (monomorphic, polymorphic, megamorphic); array iterations on the fast path and through the protocol; the ten sites with the most misses, by function, instruction index, opcode and key. Needs a build with the `ic-stats` feature; without it `qjs` rejects the option |
| `--disassemble` | The register code of every function `FILE` compiles to, root first, with its parameter, local, register, constant and cache-site counts; prints and exits without running the script |

The collector options change what the statistics show: `--min-heap-mib N` sets the floor of the reserved-bytes budget (default 1), `--gc-growth F` the bytes allocated between collections per live byte (default 2), and `--heap-cap-mib N` caps the heap. A `gc-stress` build adds `--gc-stress=N`, which collects before every N-th allocation.

Rust embedders read the same reports from `Runtime`: `gc_stats` and `gc_report`, `memory_report` (a `MemoryReport` with one field per arena and the code, register and cache counts), `ic_report(worst)` (`None` without the `ic-stats` feature) and `disassemble(source, filename)`.

## Instruction counts with Callgrind

Exact per-function instruction counts come from Callgrind on a plain release `qjs`; hardware sampling of `instructions:u` attributes samples unevenly to the functions near stalls, so it is used for totals (`perf stat`) only. `qjs` evaluates on its own declared-stack thread, so it runs under Valgrind unchanged.

```sh
valgrind --tool=callgrind --callgrind-out-file=workload.callgrind ./target/release/qjs workload.js
callgrind_annotate workload.callgrind | head -40
```

`scripts/benchmark/bench.py probes` applies the method to fixed workloads ([Diagnostic probes](performance/probes/README.md)). Each probe is a `work(n)` loop with a checked result, run under Callgrind at N, 2N and 4N: the slope (2N − N) / N is the per-iteration instruction count, (4N − 2N) / 2N checks linearity, and the same difference of per-function self cost splits the iteration by function. "Net" values subtract the empty loop's figure.

```sh
python3 scripts/benchmark/bench.py probes --qjs ./target/release/qjs \
  --v8 "$V8_V7_DIR" --output "$OUTPUT/probes"
```

`probes` covers binding, property, array, call and construction probes and the object-literal, `new` and empty-loop allocation probes; `--v8` adds fixed-iteration V8 v7 benchmark bodies under cache simulation and reports their instructions, data writes and per-function self cost; `--v8-case CASE[=ITERATIONS]` selects cases. The report is `OUTPUT/probes.json`.
