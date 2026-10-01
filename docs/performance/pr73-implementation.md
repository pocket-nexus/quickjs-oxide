# PR #73 implementation ledger

Base: `e99942a1db2e837a9d9f57ccf1fa2f1d5ca17c1a` (local verified PR #73 head).
Remote verification and publishing are blocked by Lody identity connectivity.

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
- Validation: normal library 2016 passed; profiling-feature library 2232 passed. Rollback,
  closure/eval/suspension, recursion, argument and receiver tests passed.
- Source work removed is established; timings and generated-code acceptance remain pending.

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

## Measured stage 2 result

Eight paired fixed-work process samples per case, ABBA/BAAB order, pinned CPU 2.
Target median-ratio geomean: 0.9642 (3.58% less time). DeltaBlue -9.26%, Richards
-4.96%, RayTrace -2.16%, Earley-Boyer -1.58%, Splay +0.35%. Controls: Crypto +0.74%,
RegExp -1.98%, Navier-Stokes +0.99%; combined -0.94%. All expected output matched.
See the stage2 fixed results and bootstrap intervals in the adjacent receipt directory.
These are stage2 results, not results for the subsequently stacked changes.

## Remaining acceptance

In-place heap finalization;
full frozen Test262 vector and paired workload/Score/RSS measurements against #73.
