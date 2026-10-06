# E3: canonical frame operands and direct consumption

This experiment follows E1/E2 on `perf/verified-loop-experiment`. Its ordinary
release baseline is `afadba63`, Rust 1.88, without PGO. The production B/C
migration remains paused. Uncaptured, non-TDZ locals in a separate value array
are outside this experiment.

## Ownership and borrowing

- One reusable `Vec<JsValue>` owns operands for all frames. Binding storage
  remains separate. Each frame has an operand base, reserved end and depth.
- `[base, base + depth)` owns values. The remaining reserved suffix contains
  only Undefined or immediate values: it owns no references, including atoms.
  Numeric reductions may leave inactive immediates. Moving an owning value uses
  replacement; there is no second representation or stack synchronization copy.
- Installation reserves both stores before consuming caller inputs. Ordinary
  arguments move from the operand tail into binding storage. Native consumers
  retain their current direct-completion behavior; owning native argv transfers
  retain their existing reusable buffers and cleanup responsibilities.
- A current-frame slice borrow excludes frame installation, storage growth and
  reentry. Local depth is published before frame transitions, callbacks,
  suspension, errors, GC service and cleanup. Scope exit publishes it on failure.
- Binding semantics (TDZ, capture, private values) remain intact. All storage
  accesses remain safe Rust with slice bounds checks.
- The current collector finds external roots using strong counts minus heap
  edges, rather than scanning SlotStore. Active operands must retain their
  owning edges across collection and callbacks. Suspended execution publishes
  its edges using the existing freeze/thaw protocol.
- Debug checks at frame retirement and tests reject reference-bearing inactive
  slots. Cleanup releases only the active operand prefix, plus binding owners.

## Three steps and gates

1. **Canonical storage:** move existing producers, consumers, ordinary and
   constructor calls, native argv, frame handoff and retirement to the operand
   vector. Test reservation/retain failures, argument order and aliasing, operand
   roots through collection and caller roots through callbacks.
2. **Execution slice:** acquire current-frame slices once, use local depth and
   publish through a scope guard. Test error, callback, GC, frame switch and
   suspend/resume boundaries without copying operands.
3. **Hot consumers:** directly consume numeric operands and binding slices,
   avoiding repeated classification, offsets, occupancy checks and Result
   transport where the published instruction contract proves stack effects.

Commits 1 and 2 require correctness and R/D/NS Ir growth no greater than 1%
relative to the preceding verified version. They have no speedup requirement.
Commit 3 assesses the mechanism and gains: targets are at most 40 Ir per
dispatched bytecode and 400–500 Ir per numeric-loop iteration. A 10–20% R/D
improvement remains an unverified hypothesis.

Use the existing authenticated fixed-work probes and V8 replay tools. Attribute
instructions to mutually exclusive instruction-address ranges; source line zero
and inlined source labels are not independent cost buckets. Ir/Dr/Dw provide
mechanism evidence. Native feedback uses one ABBA block, without concurrent
builds or correctness tests; small changes remain unresolved. I1 misses are
diagnostic, not a zero-growth gate. No host frequency changes or layout tuning.

Results and any unmet targets belong in the experiment report; infrastructure
commits alone do not establish a performance gain.

## Checkpoint

The three steps and a measured metadata refinement are implemented. Steps 1/2
pass their correctness and Ir gates. Step 3 does not meet the 400–500 Ir/iteration
or 40 Ir/dispatch targets; the native short block does not establish a speedup.
The implementation remains on the experiment branch. See
[the E3 report](verified-loop-e3.md) for all measurements and remaining costs.
