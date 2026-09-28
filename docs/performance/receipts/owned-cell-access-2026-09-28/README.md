# Owned captured-cell replacement: access and release evidence

This receipt compares the typed-arena parent `eebab2126b2f33bde35b61f2df1ef898317cdd70` with owned-cell replacement `c31a22a87e78d31c6ba9fcb13fd1dc83370414b4`. The change resolves the cell mutably once, validates its metadata against the incoming owner, and replaces through that same access. On failure the cell and incoming owner remain unchanged; on success the displaced owner returns to the caller for release. The retaining replacement contract is unchanged.

## What the release build establishes

The two clean AArch64 macOS plain-release `qjs` builds used Rust 1.96.0, fat LTO, and the same build script and codegen settings. Their executable SHA-256 values are **identical**: `48d71ac166f45ce2cb23920b2cf767f8ac4f37d97a5e62616c6fd2c63f5e2c24`. The two `Runtime::write_var_ref` symbol disassemblies also match after removing only the binary path header. The compiler has already erased this source-level difference in the measured build; the PR improves the scoped access contract but does not change this release executable. [Parent](data/parent-build.json) and [candidate](data/candidate-build.json) build receipts preserve source commits, binary hashes, and effective release settings.

`Runtime::write_var_ref` still performs a separate immutable private-kind preflight before calling the owned replacement. Folding that check into the same heap access would require preserving its `RuntimeError::Invariant` boundary and owner cleanup; this PR does not change that contract. The exact executable comparison is stronger evidence about *this* optimization than a noisy process timing ratio.

## Fixed captured-cell workloads

The [manifest](fixed-manifest.json) fixes a 262,144-write numeric captured cell and a 16,384-cell object-valued update case. Every process produced the exact expected stdout and exit status. Four same-binary A/A repetitions per label and eight parent/candidate A/B repetitions per label used balanced order and whole-process macOS counters.

| Workload | A/A wall span | A/B candidate ÷ parent wall | A/B instruction ratio |
|---|---:|---:|---:|
| Repeated numeric cell write | 129.6% | 0.969 | 1.0001 |
| Object-valued cell updates | 38.5% | 0.940 | 1.0004 |

The wide A/A spans and identical binaries make the A/B wall ratios non-causal. Counters include process startup, compilation, execution, and teardown. The [A/A](data/fixed-aa.json) and [A/B](data/fixed-ab.json) result files retain every sample; their raw stdout, stderr, and `/usr/bin/time -l` files are in the [archive](data/fixed-raw.tar.gz) with [SHA-256 index](data/fixed-raw-index.json).

## Bounded V8-v7 fixed-iteration comparison

The pinned external suite source is `2034d98fc8c5f8044e186267593f5d5ea5232caf`. This is the repository's fixed-iteration driver, **not** the upstream adaptive V8 Score. Its hard deadline is 540 seconds per invocation, with exact completion markers, same-binary A/A, and balanced A/B samples. The pilot reduced formal repetition to two per engine to fit the budget; the same [frozen plan](data/v8-freeze-plan.json) is replayed for the cumulative #52 comparison.

The typed-parent versus owned-cell run completed all 72 scheduled samples. All eight isolated suites and the combined case were inconclusive against their same-binary A/A spans, as expected from the identical binaries. Its [result](data/v8-typed-owned.json) records the per-suite ratios and raw counters.

The [#52-to-candidate replay](data/v8-pr52-owned.json) also completed 72/72 samples from the identical frozen plan. Its isolated-eight geometric mean is 0.970 for #52 ÷ candidate wall time, while the combined case is 1.260; three of nine cases lie outside their observed A/A spans, and the directions disagree. A/A spans range from 5.3% to 92.7%. This is a cumulative #52-to-current comparison that includes the intervening VM and storage work, and it does not attribute a performance effect to the owned-cell source change. The small fixed-iteration, high-noise replay is not an engine-wide admission result.

The frozen plan records generated-driver and upstream-body hashes; the JavaScript bodies remain in the pinned external checkout. Both V8 runs preserve their raw process records in the [archive](data/v8-raw.tar.gz), indexed by [SHA-256](data/v8-raw-index.json). The comparison is a scoped whole-process replay; it does not establish an embedded-target result or an engine-wide throughput change.

## Validation

Rust 1.88 workspace/all-target tests passed (1,976 main-library tests, 908 oracle passes and one ignored). Five production Clippy commands, formatting, source-layout and documentation checks, and focused Test262 (6,844/6,844) passed. The private-cell rejection test proves that an invalid incoming owner is returned without a refcount change or cell mutation. The PR validation record carries the full Test262 outcome separately.

## Reproduction

Use clean detached worktrees at the parent and candidate commits above. Build plain release CLIs with `scripts/benchmark/build.py --plain-only` and the same Rust toolchain, then run `scripts/benchmark/fixed.py` against the manifest with `--repeat 4` for A/A and `--repeat 8` for parent/candidate, `--order abba-baab --darwin-counters`. The build receipts record both binary hashes and effective release configuration.

Run `scripts/benchmark/iterate_v8.py` from the candidate checkout against a clean external V8-v7 checkout at the pinned commit. The first invocation used `--target-seconds 0.25 --repeat 4 --order abba-baab --deadline-seconds 540 --sample-timeout 45 --darwin-counters`. The pilot froze two actual repetitions and ABBA order. Replay the checked-in [freeze plan](data/v8-freeze-plan.json) for #52 with `--plan docs/performance/receipts/owned-cell-access-2026-09-28/data/v8-freeze-plan.json --repeat 2 --order abba --deadline-seconds 540 --sample-timeout 45 --darwin-counters`. Both output directories must be outside the repository, as required by the runner.
