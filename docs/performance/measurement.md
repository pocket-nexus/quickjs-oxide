# Performance measurement method

This page uses the repository's [`scripts/benchmark`](../../scripts/benchmark/README.md) tools and the [fixed workload matrix](probes/README.md). Each comparison records the full source SHA of candidate and reference, tool versions, build configuration, binary SHA-256, workload SHA-256, host information, raw samples and output validation.

## Builds and sampling

Plain release builds are used for time and hardware counters. Cache events come from a build with the `ic-stats` feature (`qjs --ic-stats`); collector and memory figures from `qjs --gc-stats` and `--memory-report` on a plain build. Both sides use the same toolchain and build configuration, each with its own target directory. `build.py` produces identity receipts for source, tools and binaries.

```sh
python3 scripts/benchmark/build.py --repo "$SOURCE_TREE"   --plain-only --jobs 2 --plain-target "$OUTPUT/plain"
python3 docs/performance/probes/build_fixed_matrix.py --check
python3 scripts/benchmark/fixed.py   --manifest docs/performance/probes/fixed/manifest.json   --workload-dir docs/performance/probes/fixed   --engine reference="$REFERENCE_QJS" --engine candidate="$CANDIDATE_QJS"   --repeat 8 --order abba-baab --output "$OUTPUT/fixed"
```

Sampling first establishes time resolution with an A/A comparison of the same binary, then runs interleaved A/B comparisons. Each program fixes its guest workload, and every run validates stdout, stderr and exit status. Applications that write use fresh application state on every run; property experiments start under the recorded cache policy. Fixed iterations, whole-process time, compile probes, the formal V8 v7 score and profiling counts are named separately.

## Result fields

| Cost | Recorded |
| --- | --- |
| Execution | Fixed-workload time, retired instructions and cycles where available, hot PCs and completion/handoff counts |
| Preparation | Compile time, allocation count and bytes, publication time |
| Representation | Execution words, metadata, generated code and binary size |
| Lifetime | Frame and heap capacity, owner retention and release, collection queues and peak memory |

Result tables list sample count, median, paired ratio and A/A range per item. Property reads can be measured separately for cold, stable, accessor and shape-changing cases; ordinary assignment covers call results and property results. Compilation and execution use separate measurement entries. Raw samples, result tables, profiles, build logs and experiment archives are kept in `target/` or outside the repository.

## Semantic checks

Under the same workload, compare output, exception identity, effect order and resource cleanup. Rust workspace tests, Test262 and targeted ownership tests cover language behavior; performance samples use plain builds and keep per-run raw data. The current commands are in the [status page](../status.md#verification) and the [benchmark tools guide](../../scripts/benchmark/README.md).
