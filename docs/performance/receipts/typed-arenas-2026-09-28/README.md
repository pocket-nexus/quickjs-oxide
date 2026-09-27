# Captured-cell and shape arena receipt

This is one workstream stacked on [PR #53](https://github.com/pocket-nexus/quickjs-oxide/pull/53), with a complete captured-cell path
followed by a complete shape path. The frozen-checkpoint implementation is
`ca70c00e1046a8fe7636af2697ce390854528289`; its two storage commits are
`161ae860` (cells) and `b1c23e18` (shapes), followed only by a mixed-cycle
test. After #53 advanced, the work was rebased onto its new head
`38e9eb86e2c2db8fb2e607c9f3ff611407941b4b`; the corresponding storage
commits are `4eedc2aa` and `4d79a7e3`. The rebased integrated build was
measured at `0a529c3b0eb933483f6a8cd90354c64aea2b996a`. The plan and
architecture are in [typed-arenas.md](../../typed-arenas.md). The compressed
[frozen implementation patch](data/frozen-implementation.patch.gz) preserves
the earlier `src/` delta against `10262309` (SHA-256
`67907842bd3a3219cc52e4ad17e6d336890d91f5ff48127729de258c5646fd41`).
Subsequent receipt-only changes do not change the release engine.

## Identities and method

| Build | Source commit | Plain `qjs` SHA-256 |
| --- | --- | --- |
| PR #52 cumulative | `996663f771afdabdc69d52c94bd4d2fb392e27b1` | `18488063cc3d688f47fc5e540444e16d1f3b8bab7efbf1cf3cb4c453c8f60868` |
| Frozen PR #53 | `10262309c580c43ba984b4f371acdc081147b4f0` | `acf96c92b9af7f5c03508ef9e2d3b7e0a91777c742aaada74a7ed6109097e040` |
| Cell-only | `161ae860` | `aeac0f3b3a75943f0004447645b3244e9f7c4bbb6a207f4051589d3e0ad6b26c` |
| Cell + shape | `ca70c00e1046a8fe7636af2697ce390854528289` | `52e6f666218921ac328d2bdf05545580ef49e2b1543d7b67544b53b150312f1c` |

Every source was a separate clean worktree. [Build receipts](data/builds/base-plain.json)
include all eight plain/profiling identities, source trees, binary hashes,
tooling hashes, commands and effective compiler flags. Builds were serial on
the same Apple M1 (8 logical CPUs, 16 GiB, macOS 26.6.2, AC power) with
stable Rust 1.96.0, ordinary release, fat LTO and one codegen unit. Profiling
builds were used only for accounting, never for timings. The host had other
active sessions and nontrivial load during compilation and early validation;
timing quality is evaluated separately below.

The eight checked [workloads](workloads/cell_live.js) and their hashes/expected
stdout are frozen in [fixed-manifest.json](fixed-manifest.json). Each binary
produced the same expected stdout with no script stderr. CLI snapshots were
taken after pending jobs and before Context drop. `initialized` is the CLI
`-q -dT` snapshot; `drop` assigns the global root to null; `reuse` allocates
a second population after that release. The recorded
[profile JSONL](data/profiles/combined/cell_live.jsonl) includes memory,
compile/VM cost and arena-backing allocation events for every build and
workload.

## Storage result

On this exact AArch64 release build, the shared slot remains **280 bytes**.
The dedicated captured-cell slot is **48 bytes**, the shape slot **120 bytes**,
and the unchanged leaf slot **24 bytes**. These sizes come from
`used_bytes / initialized slot count` in the matching profile builds, not
an older source comment. Object, Context and FunctionBytecode still use the
shared arena.

The table sums reserved capacity for shared, cell, shape and leaf slots, all
four free lists, and the heterogeneous zero queue. It does **not** claim total
heap/process memory: object payload backing, atoms, allocator overhead and
other categories are outside this sum. PR #52 has the same value as frozen #53
at every listed checkpoint; its raw profiles are retained separately.

| Workload/checkpoint | #53 bytes | Cell-only bytes | Cell + shape bytes | Combined vs #53 |
| --- | ---: | ---: | ---: | ---: |
| Initialized | 149,568 | 80,960 | 96,320 | -53,248 (-35.6%) |
| 4,096 live cells | 4,784,272 | 2,883,728 | 2,899,104 | -1,885,168 (-39.4%) |
| Cell roots dropped | 4,980,736 | 3,063,808 | 3,079,184 | -1,901,552 (-38.2%) |
| Cell allocation/reuse | 4,964,352 | 3,047,424 | 3,062,800 | -1,901,552 (-38.3%) |
| 2,048 object-valued cell updates | 4,784,416 | 4,981,024 | 4,996,384 | +211,968 (+4.4%) |
| 4,096 objects sharing few shapes | 2,300,080 | 2,303,152 | 2,318,528 | +18,448 (+0.8%) |
| 4,096 distinct shapes | 4,593,808 | 4,596,880 | 3,286,176 | -1,307,632 (-28.5%) |
| Distinct-shape roots dropped | 4,708,416 | 4,711,488 | 3,400,768 | -1,307,648 (-27.8%) |
| Distinct-shape allocation/reuse | 4,675,648 | 4,678,720 | 3,368,000 | -1,307,648 (-28.0%) |

The object-valued update case crosses the same shared-arena capacity step in
both builds, so its separate cell/shape vectors add capacity without lowering
the shared vector. The shared-shape case similarly shows the fixed extra-arena
cost. These are real counterexamples to a per-node-only savings calculation.
In the intended populations, first allocation, root drop and a second reuse
cycle retain the savings; the second cycle does not grow the traced slot
backing beyond its first peak.

Trace storage IDs are shared=1, leaf=2, cell=3 and shape=4. For the live-cell
case, #53's traced common-arena peak is 4,587,520 bytes; cell-only peaks at
2,293,760 common + 393,216 cell bytes. For distinct shapes, combined peaks at
2,293,760 common + 983,040 shape + 3,072 cell bytes, versus 4,587,520 common
in #53. Every trace is `finished`, has zero dropped events and ends with
backing frees. The trace covers arena `Vec` capacity changes only; it does not
observe shape-owned allocations. Combined profiling separately reports
41,192 bytes of shape entry-vector capacity and a 4,816-byte lower bound for
lookup backing in the distinct-shape case. The shape payload and lookup policy
were not redesigned, but #53 did not expose these nested categories, so this
receipt does not subtract an unmeasured baseline value.

An explicit-GC [probe](scratch-probe/src/main.rs) uses the public embedding
API and the same Rust 1.96 release settings with a locked dependency set;
its binary SHA-256 is
`2ae97e8f3251aa2c786c2a5a43dbbd81568479ca0cbbdcc5e53fc25049607ab4`
and its [build log](data/scratch/build.stderr.log) is retained.
Its raw [cell](data/scratch/cell.tsv) and [shape](data/scratch/shape.tsv)
records list phase, shared/cell/shape capacity, peak scratch capacity, live
cell count and live shape count. Peak scratch reaches **174,858 bytes** after
collecting the rooted cell graph and **174,813 bytes** after shape reuse.
Both workloads return to their prior logical cell/shape populations after
root removal and collection, while slot backing is retained for reuse. This
is a collection-local high-water observation, not resident storage to add to
the table. Frozen #53 did not expose the corresponding peak, so a baseline
scratch delta is unmeasured.

## Execution and validation

Plain-release timings use `scripts/benchmark/fixed.py` with checked stdout,
empty stderr, ABBA/BAAB order, eight samples per engine, and macOS
`/usr/bin/time -l` counters. The four workload programs in
[large-fixed-manifest.json](large-fixed-manifest.json) scale cells/shapes to
32,768 and object-valued cell updates to 16,384. They were added because the
4,096-node processes finish in roughly 16–38 ms, close to observed process
noise. The 4,096-node [A/A](data/timing/aa-combined.json) and all four
[pairings](data/timing/base-combined.json) remain as raw sample records;
their small wall-time differences are not treated as resolved. The larger
[A/A](data/timing/aa-large.json) has median label differences of 0.1–1.4%.
All 576 small and 288 large samples have status `ok` and valid counters.

The larger [#53/combined comparison](data/timing/base-combined-large.json)
has these within-pair medians. RSS and retired instructions cover the entire
process, including startup, source compilation, execution and teardown. They
do not isolate one heap helper or an embedded target.

| Workload | #53 wall ms | Combined wall ms | Wall ratio | Retired-instruction ratio | Maximum RSS MiB, #53 → combined |
| --- | ---: | ---: | ---: | ---: | ---: |
| 32,768 live cells | 133.35 | 128.40 | 0.963 | 0.986 | 35.95 → 29.48 |
| 32,768 distinct shapes | 86.64 | 81.96 | 0.946 | 0.981 | 55.88 → 41.82 |
| 32,768 objects sharing shapes | 88.33 | 84.19 | 0.953 | 0.986 | 17.38 → 17.26 |
| 16,384 object-valued cell updates | 192.20 | 184.13 | 0.958 | 0.989 | 33.38 → 30.48 |

The [cell-only comparison](data/timing/base-cell-large.json) has wall ratios
0.975 for live cells and 0.976 for object-valued updates. The incremental
[shape comparison](data/timing/cell-combined-large.json) has wall ratio 0.960
and retired-instruction ratio 0.973 on distinct shapes. Combined/#52 wall
ratios are 0.949–0.967 in the
[cumulative comparison](data/timing/pr52-combined-large.json). These runs
show no large-probe execution regression, but the exact small percentages are
not an engine-wide throughput claim. The host ran other sessions earlier in
the series, A/A and individual samples show noise, and the counter scope
includes compilation and teardown. No call-stack attribution, branch/cache
events or isolated execution-only time was collected.

Rust 1.88.0 workspace/all-target tests pass at the measured code commit,
including 1,963 main-library tests and the focused mixed
shape/prototype/property cycle. Workspace doc tests, the `test262-host`
feature/test variants and profiling CLI tests also pass. The focused Test262
vector matches 6,844/6,844,
and the full vector matches its frozen 80,010 passes of 80,060 runnable cases
(102,037 total variants). Source-layout, formatting, documentation metrics
and the five production Clippy commands from CI pass. The extra
`cargo clippy --workspace --all-targets -- -D warnings` diagnostic hits the
pre-existing `items_after_test_module` warning in `engine/vm/execute.rs`,
which this branch does not modify; that diagnostic passes with only this
warning suppressed. Validation also included the repository's authenticated
Test262 input check and the scratch-probe format check.

No embedded-target build was available for this receipt. The measured
capacity change is an AArch64 result; allocator overhead and whole-process
memory need their own target-specific measurement.

## Integration with the current #53 head

PR #53 advanced to `38e9eb86e2c2db8fb2e607c9f3ff611407941b4b` while
this work was in progress. Clean worktrees of that head and the rebased arena
candidate `0a529c3b0eb933483f6a8cd90354c64aea2b996a` were rebuilt with
the same Rust 1.96.0 release settings. Their plain `qjs` SHA-256 values are
`7ef1a8d72e15c2f1557ceffd1a861fb2e0113cc9c40278a240f73096aab2169a`
and `65d8769d701c74e8926cbe1e163696c6f5744e9db993a8ae76658157bcf8b605`.
The [base](data/builds/m2base-plain.json) and
[candidate](data/builds/m2combined-plain.json) build receipts include the
plain and profiling builds; the corresponding [base](data/profiles/m2base/cell_live.jsonl)
and [candidate](data/profiles/m2combined/cell_live.jsonl) JSONL records cover
all nine storage checkpoints. Every workload again produced the checked
stdout and no stderr. All allocation traces finished without dropped events.

The current-head slot sizes and all nine summed-capacity values are **exactly
the same** as the frozen #53 versus combined columns above: 48-byte cells,
120-byte shapes, 280-byte remaining shared slots, 39.4% less reserved slot/
free-list/queue capacity for 4,096 live cells, and 28.5% less for 4,096
distinct shapes. The object-valued update and shared-shape capacity increases
also persist. This is a second build and profile, not an extrapolation from
the old binary.

The large-workload [A/A](data/timing/timing-m2-aa-large.json) used four samples
per label; [base/candidate](data/timing/timing-m2-base-combined-large.json)
used eight per engine with the same checked manifest and ABBA/BAAB order.
All 96 samples passed. Whole-process medians on the current #53 head are:

| Workload | Current #53 wall ms | Arena candidate wall ms | Wall ratio | Retired-instruction ratio | Maximum RSS MiB, #53 → candidate |
| --- | ---: | ---: | ---: | ---: | ---: |
| 32,768 live cells | 134.11 | 127.31 | 0.949 | 0.989 | 35.84 → 29.47 |
| 32,768 distinct shapes | 86.65 | 81.85 | 0.945 | 0.982 | 55.70 → 41.81 |
| 32,768 objects sharing shapes | 88.04 | 83.76 | 0.951 | 0.984 | 17.26 → 17.24 |
| 16,384 object-valued cell updates | 192.73 | 185.00 | 0.960 | 0.987 | 33.34 → 30.35 |

The A/A label wall medians differed by 0.2–4.5%, largest for the shared-shape
case, so its wall ratio is not resolved beyond host noise. All counters include
startup, compilation and teardown; they do not isolate VM execution. The
integrated Rust 1.88.0 workspace/all-target suite passes, including 1,974
main-library tests and 908 oracle passes with one ignored case. Profiling,
doc, `test262-host`, compiled-oracle inventory, all five production CI Clippy
commands, source layout, formatting and documentation gates also pass on the
rebased branch. Its full Test262 replay matches the frozen vector exactly:
80,010 passes of 80,060 runnable cases (102,037 total variants).
