# V1: integrated phase-two validation

The frozen phase baseline is `90c85e3d94cd66066fe1c4381608d497f1dab346`
(PR #53). The integrated implementation is `85aa19565bba1c6a760cf737f74bf00a2c8df585`
(V0, A1, B1, A2 and B2). V1 changes only benchmark analysis and receipts.
Both are plain, fat-LTO release builds without the profiling feature. Their
binary SHA-256 hashes are `c8f6274b90511fa0e3446d87f825ec2f56402b1c1c8dbdb4f39aa2b41d9eb46a`
and `45ccc5046c2a446e575a00ba7c9c8646b4ba66b65c3d9bf1cededa18cef8bff1`.
The full build identities, source trees, toolchain, CPU snapshot, workload
hashes, expected output and every process result are in [data](data). Each
earlier PR receipt preserves its *local* comparison against its parent; the
tables here compare the final implementation directly with the frozen baseline.

## Application transfer holdouts

The V0 manifest freezes three independent RealWorld app implementations:
Vue (`f7e48c81`), React (`969709a3`) and Solid (`f6e77ecd`). Each uses 60
seeded articles and seven state/navigation phases. Vue and Solid mount their
actual app components and stores. React renders its page components and state
helpers, with route data supplied by the adapter; its network layer, loader
and form actions are outside this workload. The adapters, build recipe and
limitations are in [realworld/README.md](../../../../scripts/benchmark/realworld/README.md).
All 72 baseline/candidate processes and all 72 same-binary A/A processes
produced the frozen exact outputs. Each engine had 12 independent processes
per app in balanced ABBA/BAAB order. `paired_report.py` groups two complete
repetitions per block and reports the mean log ratio over six blocks. The
interval is descriptive; correlated host drift can make it too narrow. A/A
is a separate drift control, not a formal confidence interval.

| Application | Integrated / baseline whole-process time | Descriptive 95% block interval | Same-binary A/A |
| --- | ---: | ---: | ---: |
| React RealWorld | 1.0137× | [1.0005, 1.0270] | 1.0027× |
| Solid RealWorld | 1.0077× | [0.9950, 1.0205] | 1.0002× |
| Vue RealWorld | 1.0035× | [0.9927, 1.0143] | 1.0037× |

Lower is better. The React result suggests a small regression. Solid and Vue
are unresolved at this resolution. These are first-use holdout results; no
engine selection or VM implementation was changed in response to them.
Application state is recreated in each process. The measured scope includes
startup, compilation, execution and teardown; it does not claim a standalone
execute-only speedup.

## Integrated V8-v7 fixed work

The pinned source commit is `2034d98fc8c5f8044e186267593f5d5ea5232caf`.
`iterate_v8.py` piloted on the baseline, froze the number of `run` calls for
each original Benchmark, then ran the exact generated bodies in four balanced
fresh processes per engine for both A/A and A/B. All 144 scheduled processes
completed with the required marker. The freeze plan and generated-program
hashes are in [data](data). The eight isolated-suite geometric mean was
**1.0060× integrated/baseline time** (equivalently 0.9941× baseline/candidate
speed). The actual combined process was **0.9989× integrated/baseline time**.
The combined 0.1% apparent improvement is inside the 1.6% observed A/A span.
The isolated-suite result also should not be promoted into an original V8
score or a product-wide claim. Navier–Stokes was 1.0222× slower in this
cumulative run, within its 3.2% A/A span. All eight isolated interpretations
were inconclusive under the runner's observed A/A-span rule.

| Frozen V8 workload | Integrated / baseline time | Userspace retired instructions | Observed A/A time span |
| --- | ---: | ---: | ---: |
| Richards | 1.0132× | 1.0034× | 4.08% |
| DeltaBlue | 1.0127× | 1.0034× | 7.25% |
| Crypto | 1.0036× | 1.0054× | 1.13% |
| RayTrace | 1.0033× | 1.0020× | 1.96% |
| EarleyBoyer | 0.9895× | 0.9903× | 1.38% |
| RegExp | 0.9954× | 1.0008× | 4.37% |
| Splay | 1.0084× | 1.0054× | 2.17% |
| Navier–Stokes | 1.0222× | 0.9921× | 3.17% |
| Actual combined process | 0.9989× | 0.9993× | 1.61% |

The Navier–Stokes diagnostic retired fewer instructions while its wall time
rose; the two measures must not be treated as interchangeable. Instruction
counts came from a separate three-process-per-engine run, not the timed V8
series.

The original adaptive combined V8-v7 score was attempted separately. The
frozen baseline exceeded the runner's 120-second per-process limit on its
first `all` sample; the run was stopped after that timeout, so **no original
score is available**. A partial or timed-out run is not assigned a score.

## Correctness, mechanism and resource scope

The focused Rust differential tests in A1–B2 check fallback effect order,
ownership, aliases, accessors, thrown identity, cache changes, overflow and
invalid intermediate entries. The final tree passed 3,112 workspace tests
(one ignored), 46 benchmark-tool tests, and Rust 1.88 Clippy with warnings
denied. The authenticated Test262 focused vector passed 6,844/6,844 variants.
The authenticated full Test262 gate matched its pinned vector: 80,010 passes
among 80,060 eligible variants, including the same 50 eligible failures as
the frozen reference. It reported 102,037 total variants, 3,502 unsupported
and 18,475 skipped. The full TSV is compressed in [data](data), alongside the
runner provenance and console log. This is a matching compatibility vector,
not a claim that the whole corpus passes.

The successful local mechanisms are quantified in the parent receipts:
A1 retains ordinary specializations on misses; B1 removes property-result
promotion and operand traffic; A2 jointly admits adjacent numeric reads;
B2 moves a promoted own-field owner into a local without replay. Those
mechanisms did not produce a convincing integrated speedup in these app
holdouts or the combined V8 fixed-work process. The B1 field-predicate
compile corpus was 1.143× slower than A1, and A2's always-missed second
getter was 1.079× slower than B1. These costs remain open optimization work.

The final plain binary is 9,111,056 bytes versus 9,066,552 bytes for the
baseline, an increase of 44,504 bytes (0.49%). Binary size is not live heap
usage. The memory and retired-instruction diagnostics below are reported
separately from the plain-build timing series.

## Compilation and resources

The public compile probe compiled the three exact frozen app bundles in 24
alternating fresh processes per engine, followed by an adjacent 24-process
same-binary A/A series. Compilation excludes Runtime/Context construction,
source I/O and teardown. All 288 samples completed. Integrated/baseline
median compile-time ratios were 1.010× React, 1.021× Solid and 1.011× Vue;
the A/A right/left ratios were 1.001×, 1.011× and 1.003× respectively.
The Solid result in particular overlaps considerable A/A drift. These are
directional startup costs, not an isolated execution gain. Full probe build
identities and process samples are in [data](data).

Linux `perf stat -e instructions` counted userspace retired instructions in
three fresh plain-build processes per engine and workload. The median
integrated/baseline ratios were 1.0085× React, 1.0058× Solid, 1.0058× Vue
and 0.9993× for the actual combined V8 fixed-work program. Every process
produced the exact frozen output. These counts support a small app-wide work
increase, but they are not timing measurements. A separate one-process
`wait4.ru_maxrss` sample yielded baseline/integrated peaks of 31,244/29,392
KiB for React, 17,480/17,724 KiB for Solid, 53,748/54,136 KiB for Vue,
and 361,580/361,772 KiB for combined V8. A single peak RSS sample is too
noisy to establish a memory win or loss.

Profiling builds inspected completed app runs for partial live/capacity
accounting. Live heap node counts were identical between baseline and
integrated builds for React (20,681), Solid (14,465) and Vue (84,390).
Arena-slot and property-slot used/capacity bytes were likewise identical.
Published executable words increased by 512, 64 and 332 bytes respectively,
while executable projection metadata increased by 12,224, 21,696 and 40,192
bytes. These category figures exclude allocator overhead and are not a sum
of total live heap bytes. `resource-results.json`, both profile-build receipts
and the raw perf/profile outputs preserve the basis for these numbers.

## Reproduction

Use the V0 application checkout/build instructions, then run `fixed.py`
against the frozen `app-frozen-workloads.json` with `--repeat 12 --order
abba-baab`. Run the same command with the final binary twice for A/A, then
run `paired_report.py` on both results. For V8, call `iterate_v8.py` with
`--target-seconds 0.15 --repeat 4 --order abba-baab --deadline-seconds 600`
and the pinned source checkout. Its `v8-freeze-plan.json` can be passed as
`--plan` for byte-identical generated work. Run the timing series serially,
without builds or conformance tests. The archived process outputs, sample
lists and complete metadata let a reviewer check the exact output and
provenance of this run, rather than inferring validity from these rounded
ratios.

The compile corpus is the same three generated app `.js` files. Use
`compile_matrix.py --metric compile --repeat 24` with public compile probes
from `build_compile_probe.py`, then repeat with the integrated probe on both
sides for A/A. For diagnostic counters and partial memory categories, use
the exact frozen workload path from the manifest or V8 freeze plan:

```sh
perf stat -e instructions -x, -o counters.csv -- /absolute/plain/qjs /absolute/workload.js
/absolute/profile/qjs -d --profile-json --profile-output memory.jsonl /absolute/workload.js
```

`resource-results.json` records output hashes, raw counts, category bases and
the single-process Linux `wait4.ru_maxrss` samples. `resource-raw.tar.gz`
contains the unrounded counter outputs and profiles. These diagnostics are
separate from the balanced timing runs.
