# Stack virtual machine

vm executes published code and coordinates function calls, binding access,
exception unwinding, backtraces, and generator and async suspension and
resumption. The execution core is an explicit-stack VM.

`SlotStore` holds the argument, local and operand windows; `FrameEntry` /
`Frame` store the code owner, PC and resume state. The driver advances
explicit frames for ordinary JS calls and internal callbacks; only genuine
host calls establish a host boundary. Generator freeze/thaw encodes and
restores frames directly.

Conversion algorithms belong to value, property semantics to object and
built-in methods to builtins. The VM runs the JS calls these algorithms
request and propagates return values, exceptions and suspension results.
heap stores reference edges of long-lived suspended data; static guarantees
of published code together with dynamic validation of resume state govern
re-entry into execution.

See [workspace architecture](../../../docs/architecture.md) for the published
word stream, call frames, slot ownership and observation boundaries, and
[current execution paths](../../../docs/performance/roadmap.md) for the
execution mechanisms.
