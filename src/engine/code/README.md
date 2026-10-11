# Code representation and publication

code connects compiler drafts, runtime linking and execution. It owns
instructions, constants, function and module drafts, source positions and
exception metadata, and published code. Changes to the instruction contract,
code layout or publication guarantees start here, since the compiler and VM
both depend on these conventions.

The publication entry point links names in drafts, retains runtime references
and publishes transactionally; failure cleanup, Runtime identity and realm
checks are part of that flow. Publication-time snapshot checks complement the
dynamic-state checks performed when a suspended execution resumes.

Published execution snapshots share `ExecCode` and constant storage and hold
the corresponding roots. Execution words live in `Rc<[Cell<u32>]>`. The VM
manages the current PC, operand stack and call state. Shared static storage
does not share identity across Runtimes, and callers keep ownership
obligations for dynamic values during calls.

[Workspace architecture](../../../docs/architecture.md) documents the layout
and publication flow.

`bytecode_publish.rs` and its private-binding submodule link names and private
bindings; `runtime.rs` performs iterative post-order publication and the
Atom/heap transaction. Publication retains atom references for property keys
and eval binding names, instantiates template objects and links RegExp
constants. A parent retains a child's root before taking it over; on failure
it releases the auxiliary atoms, child roots and materialized-constant roots
already acquired. `api/compile.rs` orchestrates compile requests and the
source-error boundary.
