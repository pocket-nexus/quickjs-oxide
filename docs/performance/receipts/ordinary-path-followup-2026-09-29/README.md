# Ordinary path follow-up: one selected read reaches its consumer

This receipt covers the revised #67–#69 draft stack on 2026-09-29. The
baseline is #53 (`90c85e3d`); staged heads are O1 `bfc8def1`, P1
`79b1a09a`, and the E1 code head `4284336c`. This stack is based on #53.
The [initial draft gate](../ordinary-path-draft/README.md)
recorded a React regression and no reduction in the principal read handoff.

P1 now returns cold data, accessor, and complete absence directly from the
ordinary cache traversal, including sites in megamorphic cooldown. An
unsupported selection goes to the canonical driver without an additional
own-slot probe. Direct own data on slot-faithful functions remains locally
selectable after lazy initialization. A non-indexed String read selects from
the defining realm's String prototype under the same heap borrow; indexed
characters continue through the allocating driver. E1 retains an owning
getter handoff and reports the remaining driver cases. The ordinary stack
consumer uses the established result call shape because an output-parameter
experiment materially slowed the focused object loops.

## Fixed-work execution

The [sample and counter record](data/measurements.json) preserves the
authenticated per-run measurements used below. All plain timing samples used
Rust 1.88.0 release binaries without
`profiling`, CPU 2, a balanced ABBA/BAAB order, and a fresh process for each
sample. Each app sample starts with fresh application state and a cold
per-process property cache. These are whole-process times including
preparation, not adaptive benchmark scores. Every sample passed the frozen
output check. Ratio is candidate time / reference time; below 1 is faster.

| Comparison | Workload | Paired samples | Median paired ratio |
| --- | --- | ---: | ---: |
| E1 / #53 | React RealWorld | 24 | 0.9577 |
| E1 / #53 | Solid RealWorld | 24 | 0.9757 |
| E1 / #53 | Vue RealWorld | 24 | 0.9979 |
| E1 / #53 | V8 Crypto fixed work | 16 | 1.0082 |
| P1 / O1 | React RealWorld | 24 | 0.9579 |
| E1 / P1 | React RealWorld | 24 | 1.0174 |

The six Node-authenticated focused programs each run 100,000 iterations.
They use the same fresh-process policy and 24 paired samples versus #53.
The median paired ratios are `value = record.value` 0.9869,
`node = node.next` 0.9939, `if (record.value)` 0.9663,
call-derived assignment 0.9929, always-effectful getter 1.0094, and
data-to-getter phase change 0.9831. The small getter slowdown and Crypto
slowdown remain visible; neither is a claimed improvement.

Eight separate `perf stat -e instructions:u` samples per binary measured median
retired user instructions of 3,669,946,905 (#53) and 3,529,548,493 (E1)
for React, a ratio of 0.9617. Crypto measured 12,842,519,008 and
12,915,382,656 (1.0057); `node = node.next` measured 539,583,478 and
557,183,996 (1.0326); call-derived assignment measured 1,607,674,044
and 1,629,724,217 (1.0137). Whole-process wall time and instructions
therefore do not move together for every workload.

## Mechanism and lifetime

Profiling binaries were built separately and were not used for plain timing.
The one-run React diagnostic counts below compare the initial E1 draft with
the revised E1 code. They are logical event counts, not time shares.

| Event | Initial E1 | Revised E1 |
| --- | ---: | ---: |
| Local named-read completions | 138,967 | 205,596 |
| Borrowed named-read completions | 71,974 | 78,889 |
| Named-read driver handoffs | 74,688 | 1,144 |
| New frame materializations | 46,283 | 46,283 |
| Frame authentication at re-entry | 353,872 | 280,328 |
| Runtime PC publications | 608,906 | 535,093 |
| Repeated publications on materialized frames | 344,331 | 344,600 |

The revised selector attempted 62,011 String-prototype selections and
completed 54,718 String length reads. It reported 1,030 accessor selections,
136 unresolved ordinary traversals, and four receiver-release drain requests.
After the read is selected, getter calls still enter the existing driver;
indexed character creation and observable cleanup also retain their
boundaries. No selected data value is discarded to repeat the cache lookup.

The generated React program still has 70,910 instructions, 850,920 inline
code bytes, and a verified maximum stack of 35. `Frame` remains 56 bytes,
`VmAction` 16 bytes, and no generic completed-operation continuation was
added. The stripped plain `qjs` binary grew from 9,391,120 to 9,409,936
bytes (18,816 bytes, about 0.20%). The maximum slot capacity (4,144),
maximum frame capacity (256), and final React heap state (20,681 live,
4,315 vacant) match #53. Copied heap roots fell from 227,679 to 220,747;
there is no evidence of retaining dead values longer in this run.

## Preparation and verification

The public compile-only probe measured 12 fresh processes per frozen file,
excluding runtime/context construction, source I/O, and teardown. Median
candidate / #53 compile times were React 1.0013, Solid 1.0280, Vue 1.0032,
and Crypto 1.0052. The sample record reports the probe's internal
`compile_ns` measurements for this comparison. These small differences are
not an attributable preparation gain; Solid's measured increase is disclosed.
The instrumented allocation probe reported *identical* allocation calls,
requested bytes, reallocations, and peak live bytes on all four files.
For React these were 328,490 allocation calls, 54,514,808 requested bytes,
29,787 reallocations, and 26,990,617 peak live bytes.

`cargo test --workspace` passed with 2,013 library tests and 908 conformance
tests (one pre-existing ignored), including cold/warm data, lazy function
slot, String prototype data-to-getter, and exactly-once effect coverage.
Profiling Clippy passed with warnings denied on P1 and the final E1 tree.

The frozen app manifest SHA-256 is
`88a7effb232534e8ba1a2fc1ae3a05ad5ec46d197475d24e1c1a9a662d9ec193`,
the Crypto manifest is
`6365a60931c686da749b61eb579144b0bdece8f5bc6667a0c92ff960d39c5f2d`,
and the focused manifest is
`f085889f224ee3d30b3ea19e1f5eaf03f1a9f7a807f2fc223f1af2e9cc15b06d`.
Plain binary SHA-256 values are #53
`608b85ea8eab7e020fe8cebdcf2229845db7bffa8b2b29ce7f33d6cd3d7c6994`,
O1 `82f23866e316935e9bb82df86962b11a3e5298756bad7c7ccc0e343c99101aea`,
P1 `5b78c2d97a692b46258a698ec68fb870f1f771c908bbb838564c94bfec9a9746`,
and E1 `8998a14135c450bd32dafc87286170858bd2d6bef0f23d29dc90e4f941e0be4d`.
The authenticated raw runner output and profile JSONL are under
`/tmp/oxide-ordinary-perf/followup-final-wide-*` on the measurement machine.
