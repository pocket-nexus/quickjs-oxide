# Execution and performance

The engine uses a single `ExecCode` word stream and a stack VM. The compiler publishes instructions, control-flow boundaries, numeric regions and bounded fast entries; `vm/execute.rs` holds state access, and ordinary calls, returns and Base construction continue in the same interpreter loop. Drivers handle the remaining property semantics, builtins, exceptions and suspension boundaries. Ordinary binding writes transfer ownership, and named property reads handle data, accessors and complete misses in one selection flow. Captured variables and shapes use their own typed arenas.

## Documents

- [Architecture](../architecture.md): module responsibilities, publication and execution boundaries.
- [Current execution paths](roadmap.md): numeric operations, ordinary writes, named property reads and continuations.
- [Execution invariants](principles.md): ownership, borrowing, positions and observable operations.
- [Typed storage](typed-arenas.md): captured variables, shapes and collection.
- [Measurement method](measurement.md): build identity, fixed workloads, profiles and resource costs.
- [GC policy](../cycle-collection.md): budget and service points for automatic cycle collection.
- [New core, phase 0](new-core-phase0.md): comparison base, language surface not carried over, collector stress mode.
- [Baseline](baseline.md): fixed-work V8 v7, Octane, AWFY and probe table against QuickJS on the reference machine.
- [New core, phase 1](new-core-phase1.md): value and heap plan for the new core tracked in #122, with milestones and acceptance checks.

## Tools

- [Diagnostic probes](probes/README.md): fixed workload matrix, ordinary-write programs, object allocation ledger and Callgrind runtime probes.
- [Benchmark tools](../../scripts/benchmark/README.md): build identity, fixed workloads, scaling, compilation, Callgrind, PGO and V8 runs.
- [Diagnostics reference](../profiling.md): report fields of the `profiling` build.

Performance conclusions are stated per measured source, binary, workload and raw samples. A change to an execution path is checked for semantics, local operation cost, whole-program time, compile cost and memory lifetime.

An optimization first states the target property, scenarios, hard constraints and acceptable trade-offs, then selects a mechanism from a profile or an established resource problem. When the existing interface supports the mechanism, only the implementation changes; when a contract blocks the target mechanism, types and responsibilities change. The mechanism, the target result and the full cost are verified separately; instruction counts and time are measured separately.

Raw samples, logs, profiles, measurement results and experiment archives are kept in `target/` or outside the repository; the repository maintains the current design, measurement method and measurement tools.
