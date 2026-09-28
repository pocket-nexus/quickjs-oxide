# Numeric-region publication and Navier–Stokes chain follow-through

Implementation: `a48fb20c` and `67a88fab`, rebased on the then-current PR #53 head `9a3df25a`. The reviewed scalar-first expression `out = scale * array[i]` is now declined by the product-store selector before publication; the array-first form remains selected. Both multiplication orders, getter and coercion fallback, and array-product updates are exercised by the numeric-region tests. This leaves valid scalar-first code on ordinary execution rather than turning an optional optimization into an encoding error.

Three small regions remove intermediate work from the executed `lin_solve` chain: an array-element copy, an add fed by `array[++index]` that consumes and replaces the stack Number, and an array write followed by a local write that consumes its saved stack target. Their opcodes select handlers directly. The original words remain authenticated and replay in evaluation order on a miss. Array-product updates reuse the target index for the source access only when publication proves the same stable direct binding. CFG lexical initialization uses precomputed bitset transfer facts and a changed-block worklist.

## Executed coverage and storage

The [final pinned Navier–Stokes profile](data/navier-final-profile.jsonl) completed one fixed Setup/run/TearDown driver. `addFields` remained 50,700 attempts and hits. `lin_solve` recorded one copy site at 16,384 hits, three add sites at 655,360 hits each, and one array/local store at 655,360 hits. `lin_solve2` recorded two copy sites at 16,384 hits each. None of those sites missed, and all profile omission counters were zero. These are logical event counts, not time shares.

The profile counts 14 common descriptors at 80 inline bytes each (1,120 bytes), one product payload at 56 bytes, and three copy payloads at 96 bytes: 1,272 inline bytes in those tables. Empty producer tables are absent. These counters exclude `Rc` headers, allocator overhead, and the `ExecCode` field storage; they are not a whole-program memory claim.

## Fixed-work V8 v7 comparison

The [paired result](data/v8-final-vs-head/results.json) compares clean Rust 1.96 plain release binaries at `9a3df25a` and `67a88fab` with the pinned V8 v7 source `2034d98f`. It uses the [frozen plan](data/v8-final-vs-head/freeze-plan.json), four ABBA/BAAB repetitions per binary in each A/A and A/B phase, and Darwin whole-process counters. The candidate CLI is 7,845,248 bytes; the current-head CLI is 7,827,664 bytes (+17,584). This is a fixed-work comparison, not the original V8 Score.

| Case | Candidate/head wall | Candidate/head retired instructions | Same-binary A/A wall span |
| --- | ---: | ---: | ---: |
| Navier–Stokes | 0.961× | 0.951× | 14.9% |
| Combined eight-suite process | 1.006× | 0.999× | 28.4% |

The Navier retired-instruction reduction supports actual work removal. Its wall change is within the observed A/A span. The combined process is effectively flat on these samples. The isolated eight-case wall-ratio geometric mean was 0.979× candidate/head; it does not replace the cumulative combined process result.

## Array-product hit and miss cost

Three experimental binaries used the same `67a88fab` implementation with the exact [forced-immediate-fallback patch](experiments/forced-fallback.patch), the [ordinary-publication patch](experiments/ordinary-update.patch), or neither. The ordinary variant suppresses only array-product region selection, allowing ordinary inner specialization. The forced variant keeps the region word layout and immediately takes its retained fallback. They are analysis builds, not shipping alternatives. Their exact binary hashes and build command are below; [raw balanced samples](data/cost-normal-vs-ordinary/results.json) and [same-binary A/A](data/cost-normal-aa/results.json) are retained.

| Probe and comparison | Wall ratio | Retired-instruction ratio |
| --- | ---: | ---: |
| Numeric hit: normal / ordinary | 0.437× | 0.405× |
| Accessor miss: normal / ordinary | 1.228× | 1.201× |
| Accessor miss: normal / forced fallback | 1.046× | 1.037× |
| Accessor miss: forced fallback / ordinary | 1.181× | 1.158× |

The [normal/forced](data/cost-normal-vs-forced/results.json) and [forced/ordinary](data/cost-forced-vs-ordinary/results.json) pairs separate additional admission work from the retained fallback representation. The later A/A miss wall median ratio was 0.855 despite the same binary, while its retired-instruction ratio was 0.998. Therefore the wall decomposition is directional under this host load; the instruction differences establish a real cost in both parts. This does not justify charging the whole miss penalty to one guard or enabling a global retry policy.

## Bounded one-borrow experiment

The [joined-borrow patch](experiments/combined-borrow.patch) reads the source Number and writes the target under one mutable state borrow, including aliased arrays. It passed all 26 focused numeric-region tests and returned the expected result on hit, source accessor, frozen target, alias, and materialized probes. The [eight-sample paired matrix](data/combined-borrow-paired/results.json) showed joined/split retired ratios of 1.000 for alias, 1.000 for frozen target, 0.992 for materialized hit, 1.001 for source accessor failure, 0.993 for the two-million-update hit, and 1.001 for its accessor miss. Wall results moved in both directions under a noisy host. The added code does not show a clear enough whole-operation gain, so it was not retained.

## Compilation and validation

Two clean compile probes measured Script compilation, excluding context construction, source I/O, and teardown. The [paired 30-sample result](data/compile-paired/results.json) used the existing 500-site [product update corpus](../execution-ready-plans-2026-09-28/compile/product-update-500.js) and the new [500-function lexical-loop corpus](compile/lexical-loop-500.js). Candidate/head medians were 1.061× (17.143/16.156 ms) and 1.045× (30.515/29.215 ms), respectively. [Candidate A/A](data/compile-aa/results.json) was 0.985× and 1.000×. This is the cost of the complete compiler patch, not an isolated measurement of the initialization worklist.

Rust 1.88 workspace Clippy passed after the final payload change. The full `cargo +1.88.0 test --locked --workspace --all-targets -q` run passed, including 1,985 engine unit tests and 908 oracle tests (one ignored). Focused Test262 matched its frozen vector: 6,844 passes of 6,844 eligible variants, with no failures or unsupported cases. The no-Test262-special-casing and performance-artifact inventory gates passed. These tests include 26 focused numeric-region cases covering both multiplication orders and hit/miss fallback behavior.

Experimental plain binaries were built with `cargo build --locked --release -q -p quickjs-oxide-cli --no-default-features --bin qjs`, using the same Rust 1.96 toolchain and shared dependency target after forcing workspace recompilation. SHA-256: forced fallback `3ae7e731b87d5e3074cd78e51a33b915bd6f0b91b8680d8ca20db3d382c682bf`; ordinary publication `d99250b11364ee3e71eac40b44147c869142cd431b6f49b5c42be29ea183ca8d`; joined borrow `6b5bdfb2bd45fa9a4d9b282326dbe2f9249b68c26462481f958e20262766d296`. Clean binary and compile-probe build receipts are in [builds](builds/). Each measurement directory preserves result JSON, per-sample records, and a lossless `raw.tar.gz` with a SHA-256 file where raw stdout/stderr/counters were produced. Absolute temporary paths inside JSON identify the original measurement host; archive entries use relative `raw/` paths.
