# A2: two admitted preincrement reads

Implementation: `deccdde5`, stacked on B1. Adjacent `AddPreInc` fragments
that consume and replace the same stack Number now form one bounded numeric
region. The compiler distinguishes the second read of a shared index from
its entry version. The published operation admits both own dense Number reads
and both numeric indices before any local or stack write, computes the two
adds in source order, then commits both indices and the final accumulator.
A second-access hole/getter declines before the first increment, and one
ordinary selected fallback body executes the original sequence. The verifier
authenticates the separate guard and fallback entries, the second producer,
and all intermediate boundaries.

The pinned V8-v7 Navier–Stokes source at `2034d98f` has three adjacent
`x[++index]` reads in `lin_solve`. The first two form this pair. A one-cycle
fixed diagnostic profile recorded **655,360 attempts and 655,360 hits** at
the pair site, with no misses. Its complete generated-workload metadata and
profile record are in [data](data). This is executed application evidence for
selection; it is not a timing score.

## Fixed-work local comparison

The B1 and A2 plain release binaries ran two Node-authenticated synthetic
development workloads and that same generated one-cycle Navier–Stokes
program. Four balanced ABBA/BAAB fresh-process samples were measured per
engine and workload. Ratios are medians of whole-process wall time, including
startup and compilation. An adjacent A/A series used the A2 binary on both
sides. Exact process samples, outputs, hashes and build identity are in
[data](data).

| Workload | A2 / B1 wall | Same-binary A/A wall |
| --- | ---: | ---: |
| Pair, both own dense Numbers | 0.975× | 1.009× |
| Pair, getter on second access | 1.079× | 1.010× |
| Navier–Stokes fixed one-cycle program | 1.017× | 0.996× |

The synthetic hit improvement is modest. The second-getter regression and
the Navier–Stokes timing loss are meaningful at this sampling resolution.
The guard still dispatches before ordinary fallback, and the combined
admission does not yet win wall time in its discovery application. The change
is therefore a correctness and mechanism milestone, not a demonstrated
integrated speedup. A quick unpaired `perf stat` probe suggested fewer retired
instructions in Navier–Stokes; it is deliberately not used as timing evidence.

The standalone Script compile probe ran a 500-function pair corpus and the
lexical-loop control in 24 alternating fresh-process samples per engine.
A2/B1 median compile-time ratios were 1.009× and 0.986×, respectively;
same-binary A/A ratios were 0.991× and 1.011×. These small changes are
unresolved. Compilation excludes runtime/context construction, source I/O,
and teardown.

The A2 plain binary is 9,098,728 bytes, 11,936 bytes larger than B1. Its
SHA-256 is `add375efec8bd33a4051ea16cb045504910fced2024a02f39a808f46f9669e6c`.
Raw archive SHA-256: A/B
`1f56b8437e8dc82e94ba1cc45b86be7ec8d4e604411050224b6bc0dcb572b3ab`;
A/A `630c04cf62c4508c5b7f06c8f9bb94638069c4e9fee02727948bcbd1fe887d5d`;
compile A/B `3eaede10e300efd7af3360bd57ee451dee44f5fbbf784f6df1942e4d9c95f4c5`;
compile A/A `0fe712a5ee4f110f23e5110cae93c154199ce9401a7aebb735764e7235cc4e17`.

Focused differential tests cover shared and distinct index locals, aliased
arrays, a getter or hole at either access, numeric overflow, signed zero,
and rejection of a jump into the composed interior. The full workspace
suite passed 2,010 engine tests and 908 oracle tests (one ignored), plus
the other targets. Rust 1.88 workspace Clippy passed with warnings denied.
