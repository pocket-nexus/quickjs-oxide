# Cycle collection policy

`Runtime::gc_policy()` and `Runtime::set_gc_policy(GcPolicy)` expose the policy.
The default is Automatic. Embedders can select Manual. Explicit `Runtime::run_gc()` always works.
Changing policy never collects or executes JavaScript.

Objects, contexts, bytecode, shapes and captured cells consume one budget unit
when published. Leaves and aborted reservations do not consume budget. Ordinary
reference-count reclamation already produces a cleanup summary; return those
cycle-node credits once per zero-queue batch while the budget remains positive.
Arena reserve/abort/reclaim methods carry no scheduler parameters or accounting.

The initial headroom is 16384. Zero latches a pending request: subsequent releases
cannot clear it. This tracks net cycle-node growth and avoids collecting merely
because short-lived acyclic objects churn. It is not a byte limit or an exact
live-node counter maintained through every arena lifecycle state.
The same minimum applies to all workloads. More headroom amortizes full-graph
scans on small heaps, at the cost of retaining more nodes between collections.

After successful explicit GC and deferred releases, count occupied cycle nodes
once and rearm `max(16384, L)` headroom. Allocation never invokes the collector.
The ready driver services requests before each operation, after internal heap
borrows have ended. Cycle-node allocations return to this driver; the resident
executor's pure loops allocate no cycle nodes. There is no GC-specific VM action,
entry poll or branch/backedge poll. New resident allocation paths must preserve
this boundary. Tests check reclamation inside long allocating turns.

The outermost execution turn also services requests after clearing kept objects.
Nested entries share that turn. Active/suspended frames, jobs and owned temporary
values keep their ordinary roots. Borrowed state, active collection and panic
unwinding defer a request without blocking JS progress. Finalization jobs are
queued without running callbacks. Existing weak processing order is preserved.

Collection is synchronous and full; it has no bounded-pause guarantee. Budget,
live nodes, arena capacity and process RSS are different measurements.
