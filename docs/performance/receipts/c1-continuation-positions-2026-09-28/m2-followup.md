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

## Paired timing and instruction follow-up

The [compile-once A/A control](data/m2-repeat-aa-execute.json) and
[C1 versus M2 run](data/m2-repeat-ab-execute.json) use the same byte-identical
probe source in both worktrees (SHA-256 recorded in each file), two balanced
process pairs per case, three warmups and nine timed `Context::execute` calls
per process. These timings exclude compilation and process startup. The
[whole-process A/A control](data/m2-repeat-fixed-aa.json) and
[C1 versus M2 run](data/m2-repeat-fixed-ab.json) use `/usr/bin/time -l`, two
ABBA-ordered samples per binary and case, and the fixed output contracts.

| Case | A/B execute-only median, baseline / C1 ms | C1 / baseline retired instructions, whole process |
| --- | ---: | ---: |
| Primitive call loop | 918.6 / 815.4 | 0.9932 |
| Primitive one-frame loop | 245.5 / 232.8 | 0.9595 |
| Number-only loop | 659.2 / 617.0 | 0.9969 |
| Object conversion | 296.1 / 287.6 | 0.9942 |
| Comparison | 120.0 / 115.1 | 0.9946 |
| M1 dense hit | 313.2 / 290.4 | 0.9834 |
| M1 miss | 1623.0 / 2150.6 | 0.9881 |

These instruction totals include every part of the process, and changes in
uncovered controls show that code generation moved beyond the deleted
recovery call. The A/A retired-instruction ratios were 0.9982 for the call
loop, 1.0000 for the one-frame loop, and 0.9993 for Number-only. The
one-frame A/B reduction therefore supports less machine work on the intended
path. It does not isolate the exact cost of one recovery decode. The original
receipt's approximately 1.0006 call-loop ratio used an older #53 baseline;
it is a distinct comparison, not an interchangeable sample.

**Elapsed-time impact remains unresolved.** On this host, the A/A call-loop
execution medians differed by 33.8% despite using the same binary on both
sides; its individual samples ranged from 1,089 to 4,014 ms on one label and
1,211 to 2,373 ms on the other. Other repository builds and tests were
running concurrently. The A/B medians above are retained as raw evidence,
not a speedup claim. A quiet-host repeat with A/A controls remains necessary
before judging runtime magnitude.

## Rebasing validation

The rebased C1 focused tests, workspace tests on the cumulative C1+C2 code,
and GitHub fast/focused checks passed. A local full Test262 run on C1 at
`ade7f11e` reported **80,008/80,060** runnable passes because the sloppy and
strict variants of `RegExp-leading-escape-BMP.js` each exceeded the runner's
fixed 30-second limit while the host was heavily contended. The
[timeout audit](data/m2-test262-timeout-audit.json) compares its body to a
report matching the frozen `current.conf` body hash: those two timeout rows
and the resulting summary are the only differences. Both variants passed in
an isolated replay with one worker and a 180-second timeout. This supports
an environmental timeout diagnosis; it **does not count as a passing full
gate at the required 30-second limit**. A quiet-host full replay remains
pending.
