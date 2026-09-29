# PR #73 implementation ledger

Base: `e99942a1db2e837a9d9f57ccf1fa2f1d5ca17c1a` (local verified PR #73 head).
Remote head rechecked successfully through the approved network boundary: PR #73 is open at the same commit.

## 1. Synchronous weak-reference lifetime

- Runtime kept-object roots for objects and symbols; constructor and successful dereference.
- Nested VM entries and explicit embedding scopes share the outer execution turn.
- Module compilation/link/evaluation and pending job dispatch cover host callback boundaries.
- Jobs cannot interrupt an active turn; errors and host unwinding clear roots.
- Validation: workspace/all-targets 2071 library tests plus integration/oracle/runner suites passed;
  one pre-existing 65K argument-spread stress test ignored. After added realm/finalizer coverage,
  focused weak tests: 42 passed. No new skip or conformance exception.

## 2. Ordinary call ownership

- Transfer callee, method receiver and unobserved arguments from caller slots after fallible preflight.
- Frame function and closure view share one callee root; detached environments keep independent ownership.
- Original arguments are retained only when published bytecode can observe them. WeakRef roots
  are independent of parameter reassignment. Mapped bindings and captured cells remain owners.
- The existing nonrecursive Call/Return driver and transaction/PC checks remain in force.
- Pure synchronous leaves and strict equality complete inside the ready loop,
  avoiding the cold dispatcher and repeated pending-state checks. Errors still
  reach the shared unwinder; materialization before observable work is preserved.
  Earley-Boyer diagnostics record 36612 pure completions and 129305 strict
  comparisons taking this path. Splay records 12139 pure completions.
- Validation: normal library 2016 passed; profiling-feature library 2232 passed. Rollback,
  closure/eval/suspension, recursion, argument and receiver tests passed.
- Source work removed is established; stage 2 measurements are recorded below.
- A fresh 100000-call `id(object)` diagnostic run reports zero parameter-value
  copies and zero parameter-heap-root copies (formerly 100000 heap-root copies),
  while preserving three allocated frame boxes and 99999 reused frame boxes.
  This does not count object reads/returns as free: their legitimate owners remain.

## 3. Bounded construction prefixes

- Up to 256 shared prefix shapes, at most 64 properties each and 256 KiB charged
  direct shape storage; FIFO eviction and fallible optional admission.
- Explicit GC and runtime teardown release every optional root, including its
  atom/prototype edges. Dictionary and larger unique shapes retain their append path.
- Library validation: 2017 passed. Lifecycle tests compare after cache flush;
  retained prefixes stay immutable and subsequent objects reuse canonical successors.

## 4. Property cache and consumption

- Four data locations, with existing shape-generation, revision, realm and prototype guards.
- Retry delay starts at 16, doubles to 256 after repeated instability, and resets after 16 hits.
- The VM consumes the selected owned value directly; cold outcomes contain no data value.
- Library validation: 2018 passed, including four/five-shape and bounded-backoff tests.

## Measured ownership-only result (before the synchronous-leaf follow-up)

Eight paired fixed-work process samples per case, ABBA/BAAB order, pinned CPU 2.
Target median-ratio geomean: 0.9642 (3.58% less time). DeltaBlue -9.26%, Richards
-4.96%, RayTrace -2.16%, Earley-Boyer -1.58%, Splay +0.35%. Controls: Crypto +0.74%,
RegExp -1.98%, Navier-Stokes +0.99%; combined -0.94%. All expected output matched.
See the stage2 fixed results and bootstrap intervals in the adjacent receipt directory.
These are stage2 results, not results for the subsequently stacked changes.

## 5. Resident heap finalization

- Resident nodes use zero strong count as the queue marker; decrementing the last
  reference no longer moves Node/ObjectData through another enum variant.
- Finalization snapshots outgoing handles and atoms, drops payloads in place, and
  preserves compact Zombie counts for cycle edge release. Generation advancement
  and weak-list unlinking retain their checked ordering.
- Shared arena slots shrink from 288 to 280 bytes on this 64-bit build.
- Production live lookups reject queued residents. Fault-injection tests repair
  synthetic zero counts through the test-only setter rather than live APIs.
- Full workspace/all-targets with profiling: 2295 library tests, 908 CLI differential
  tests and all integration/runner suites passed. The pre-existing ignored 65K
  argument spread stress test was also run separately and passed.
- Workspace/all-targets Clippy with profiling and `-D warnings` passed.
- Full Test262 on the final candidate: frozen result vector unchanged,
  80010 passing of 80060 eligible variants (102037 total).
  No skip, admission or diagnostic exception changed.
- Optimized release disassembly: `release_raw_no_drain` changes from 1739 to
  1594 bytes and `finish_node` / `finish_resident_node` from 8883 to 7411 bytes.
  Each former function contained one `memcpy` call; neither replacement does.
  This establishes generated work removed, independently of timing results.
- The ready loop grows from 14504 to 16344 symbol bytes after adding synchronous
  completion paths. Dispatch counts establish fewer handoffs; the full-stack
  timings below, rather than source size, determine acceptance.

## Final fixed-work and memory acceptance

The production candidate is `59be90f5`; later commits contain receipts and
documentation only. Baseline binary source `2cfcea2f` has the same production
sources as PR #73 head `e99942a1`. Both are clean Rust 1.88.0 release builds,
fat LTO, one codegen unit and no profiling. Full build receipts are retained.

Eight paired process samples per case, ABBA/BAAB, pinned CPU 2 on the Ryzen
7 7840HS. No build, correctness test or diagnostic profile ran concurrently.
Ratios below are medians of paired after/before process times. Confidence
intervals use 4000 paired bootstrap resamples; all workload output matched.

| Target | Time change |
| --- | ---: |
| DeltaBlue | -11.49% |
| Earley-Boyer | -6.61% |
| RayTrace | -8.78% |
| Richards | -6.69% |
| Splay | -9.12% |
| Target geometric mean | **-8.56%** |

Combined fixed workload: -5.11%. Controls: Crypto -1.08%, RegExp -0.02%,
Navier-Stokes -1.74%, React -2.36%, Solid -2.26%, Vue -4.70%.
No control has a statistically supported >3% regression.

All twelve peak-RSS comparisons pass the 5% budget. The maximum median increase
is 3.15% (Navier-Stokes); Splay is -2.26%, Earley-Boyer -3.98%, Vue -9.76%.
RSS uses an independent `wait4` child after helper exec, so Python's inherited
peak cannot mask small-workload differences. A 50 MB parent-allocation probe
confirmed that isolation. The two earlier unavailable/contaminated measurement
attempts are excluded; only `rss/` contains admitted measurements.

## Splay phase attribution

Eight paired fresh processes at each of 1, 32 and 512 `run()` iterations.
The upstream Setup/run/TearDown bodies and deterministic RNG are unchanged;
only the phase driver and `Date.now` boundaries are added. These are diagnostic
phase timings, separate from the unmodified original Score. The table reports
ratios of per-engine phase medians, with times in milliseconds.

| 512 iterations | Before | After | Change |
| --- | ---: | ---: | ---: |
| Setup | 1676 | 1514 | -9.67% |
| run | 10980.5 | 9887.5 | -9.95% |
| TearDown | 281.5 | 225 | -20.07% |

The scored body improves too; the fixed-work improvement is not attributed
solely to constructor or finalization work.

## Original Score acceptance

Eight paired all-suite processes using the unmodified original V8-v7 driver,
ABBA/BAAB order and CPU 2. All sixteen processes passed output validation.
Higher scores are better; changes are medians of paired after/before ratios.

| Original metric | Score change |
| --- | ---: |
| DeltaBlue | +12.35% |
| EarleyBoyer | +9.74% |
| RayTrace | +8.30% |
| Richards | +10.48% |
| Splay | +9.71% |
| Target geometric mean | **+10.11%** |
| Overall Score | **+6.47%** |

The overall Score ratio's paired bootstrap 95% interval is +5.81% to +7.14%.
The target geometric mean interval is +9.62% to +10.36%, resampling whole
all-suite pairs together. Controls: Crypto +1.11%, RegExp +0.06%,
Navier-Stokes +2.75%. All acceptance gates pass; none was relaxed.

Receipts, exact binary/build/workload hashes, samples and analysis scripts:
[`pr73-implementation-2026-09-30/`](receipts/pr73-implementation-2026-09-30/).

## Scope of execution-boundary work

Named data reads now consume the selected value without a second value-bearing
outcome. Ordinary calls transfer existing roots through installation. Pure
leaves and strict comparison avoid the outer cold-dispatch round trip. The
nonrecursive ready driver, verified frame transactions, entry-PC checks and
materialization before potentially observable cold operations remain intact.
This patch does not claim to eliminate all `execute_frame` reentries or all
activation registrations. The final diagnostic counts retain those costs so
remaining work is visible rather than counted as a completed optimization.
