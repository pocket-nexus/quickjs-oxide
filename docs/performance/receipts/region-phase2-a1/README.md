# A1: guarded array-product fallback

Implementation: `76bcc65f` and `56d6e606`, stacked on the V0 benchmark admission change. The
array-product update region now has a physical guard entry followed by the
ordinary selected first operation. A failed guard enters that operation without
manually emulating it. The rest of the fallback keeps eligible ordinary
specializations, including dense array reads. Normal control flow still maps
the logical source instruction to the guard; the fallback word maps to the same
source/debug position. The verifier authenticates both physical entries and
rejects a branch into an eliminated intermediate state.

The other six numeric region families keep their existing publication and
fallback behavior. Product candidates whose first ordinary selection would
consume the guarded entry are declined before publication. There is one
execution stream and one interpreter.

## Fixed-work local comparison

The frozen V0 `90c85e3d` plain release binary and A1 plain release binary
executed the previously published two-million-iteration product hit and
accessor-miss workloads. The existing `fixed.py` runner checked the frozen
source hashes, exact stdout, and exit status. Four balanced ABBA/BAAB process
blocks per workload completed. The ratios below are medians of whole-process
wall times, including startup and compilation; they are not isolated operation
costs or an original V8 score.

| Workload | A1 / baseline wall | Same-binary A/A wall |
| --- | ---: | ---: |
| Product hit | 0.997× | 0.998× |
| Accessor miss | 0.764× | 0.992× |

The accessor-miss reduction is larger than this A/A drift and agrees with the
mechanism: the first operation runs once and ordinary inner dense reads remain
selected. The hit result is unresolved at this sampling resolution. The A1
binary is 9,072,712 bytes, 6,160 bytes larger than the 9,066,552-byte
baseline binary. Exact build identity, process samples, metadata, results, and
raw output archives are in [data](data). The A1 binary SHA-256 is
`0f6eed96da8a0daceaab20e9ea66144d3ddd261cbfe1023b43ec14844e0a2782`.

The existing standalone Script compilation probe excludes runtime/context
construction, source I/O, and teardown from its timed interval. It compiled
the frozen 500-site product-update and 500-function lexical-loop corpora in 24
alternating fresh-process samples per binary. A1/baseline median compile-time
ratios were 0.970× and 0.986×, respectively. The corresponding same-binary
A/A ratios were 0.975× and 0.995×, so these small apparent gains are
unresolved. The first implementation did show a 3–6% median compile cost;
removing an unnecessary second selector pass removed it in this comparison.
The compile-probe builds and complete samples are preserved in [data](data).

The workspace test run passed 2,000 engine unit tests and 908 oracle tests
(one ignored), plus the other workspace targets. The focused numeric-region
suite passed 26 tests. Rust 1.88 workspace Clippy passed with warnings denied.
Tests authenticate the guard/fallback word boundaries, retained dense-read
selection, aliasing, accessor effects, and corrupt fallback targets.

The older [array-product cost decomposition](../numeric-region-followthrough-2026-09-28/README.md#array-product-hit-and-miss-cost)
separates admission from degraded retained fallback on the previous design.
These A1 samples compare the complete old and new implementations. They do not
isolate the cost of the added guard word; the code-size and separate compile
results above are the available layout and preparation evidence.

Raw archive SHA-256: A/B
`9001906aefad7d0c3c711f51907c81bd934181b0723de5c7e797a2cc13fff6ca`;
A/A `89f0ddeaa47d8a873b91cbcebeddf9d34c6ded26bc70571e23dacce231c2bcd5`;
compile A/B `bb8bd066d93217b279ec90a99dc1a8f49fc2520b2462f0d912f705aeb361e529`;
compile A/A `ff242027a75921fb23113951761299ae32e88e9699facdfbf426979838416bd1`.
