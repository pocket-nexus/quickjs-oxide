# Cycle collection policy

`Runtime::gc_policy()` and `Runtime::set_gc_policy(GcPolicy)` expose the policy.
The default is Automatic. Embedders can select Manual. Explicit `Runtime::run_gc()`
collects under either policy, subject to state admission and runtime quarantine.
`set_gc_policy` only stores the policy.

Objects, contexts, bytecode, shapes and captured cells consume one budget unit
when published; String and BigInt leaves are outside the budget. Ordinary
reference-count reclamation returns the cycle-node credits from each zero-queue
batch's cleanup summary while the budget remains positive.

The initial headroom is 16384 (`MIN_GC_HEADROOM` in `src/engine/heap/gc_pressure.rs`).
Zero latches a pending request until a collection completes. The budget tracks
net cycle-node growth, so short-lived acyclic churn returns its credit. The same
minimum applies to all workloads. More headroom amortizes full-graph scans on
small heaps, at the cost of retaining more nodes between collections.

After each collection, the heap counts occupied cycle nodes `L` once and rearms
`max(16384, L)` headroom. Allocation records budget use; collection runs at
service points. Execution under the held state services a request at fully
published allocation points: object and array literals, Base construction after
it publishes the receiver, arguments and saved return owner, and property writes
that publish a new layout. Every such allocation publishes its owners first. The
ready driver services requests between operations after internal heap borrows
have ended. Tests check reclamation inside long allocating turns.

The outermost execution turn also services requests after clearing kept objects
and draining deferred releases. Nested entries share that turn. Active/suspended
frames, jobs and owned temporary values keep their ordinary roots. External state
borrows, active collection and panic unwinding defer public service of a request.
Collection queues FinalizationRegistry callback jobs; the callbacks run when the
job queue is drained.

Collection is synchronous and traverses the full graph. Budget, live nodes,
arena capacity and process RSS are different measurements.
