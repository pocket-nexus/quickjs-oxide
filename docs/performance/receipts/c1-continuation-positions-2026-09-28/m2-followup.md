# C1 follow-up on the M2 #53 head

PR #53 advanced to `38e9eb86e2c2db8fb2e607c9f3ff611407941b4b` after the original C1 receipt. The rebased C1 production candidate is `5b0ded27ff2205426134a61333b0a02c0265022a`. This follow-up compares freshly built plain and profiling binaries at those commits. It preserves the [original #53 `10262309` receipt](README.md) as historical evidence; those original binaries and samples are not relabeled.

## Contract coverage

The new focused tests assert that successful direct completion commits `resume_pc` to the carried boundary while retaining the operation's `fault_pc`. A test-only counter scoped to that completion proves it did not call `Frame::next_pc()`. Multi-line direct and nested conversion throws check source location, identity and one-time effects. A compiled wide `CompareBranchStack` test now runs the generic fallback through both branch outcomes. A compiled `PostInc` action is taken through constrained output capacity and retains the original PCs after its first output succeeds and the final output fails.

The cold numeric path still matches `Numeric { kind, .. }`: it receives the field after a primitive decline but does not consume it. Numeric comparisons and object-conversion completion therefore remain unmigrated.

## Rebuilt mechanism evidence

[Build identities](data/m2-repeat-builds.json), [profile counts](data/m2-repeat-profile-counts.json), [codegen summary](data/m2-repeat-codegen.json), and [assembly excerpts](data/m2-repeat-codegen-excerpts.md) record the toolchain, binary hashes, path coverage, and selected ARM64 instructions. Both builds use Rust 1.88.0 and the repository release profile. The baseline action/result/frame sizes are 16/16/56 bytes from a temporary test-only layout probe after its clean release build; the candidate profiling binary reports the same sizes.

| Workload | Numeric exits | Carried direct completions | Legacy recovery decodes |
| --- | ---: | ---: | ---: |
| Primitive call loop | 1,000,000 | 1,000,000 | 0 |
| Primitive one-frame loop | 1,000,000 | 1,000,000 | 0 |
| Number-only loop | 0 | 0 | 0 |
| Object conversion | 100,000 | 0 | 100,000 |
| Comparison | 100,000 | 0 | 100,000 |
| M1 dense hit | 0 | 0 | 0 |
| M1 miss | 2,000,000 | 2,000,000 | 0 |

The one-frame probe is in `workloads/primitive-frame.js` and the versioned `manifest-m2-repeat.json`. The plain binaries produced the expected output for every case in that manifest. `ready::run` contains two static `Frame::next_pc` calls in the M2 baseline and one in C1; its symbol span is 10,980 bytes in baseline and 10,652 bytes in candidate. These are static machine-code observations, not retired-instruction counts or a runtime speedup claim.
