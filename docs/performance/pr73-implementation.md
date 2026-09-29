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

## Remaining acceptance

Four-entry property cache, in-place heap finalization;
full frozen Test262 vector and paired workload/Score/RSS measurements against #73.
