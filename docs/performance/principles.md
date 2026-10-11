# Execution invariants

These constraints apply to the compilation, publication, execution and collection paths of the stack VM. Module responsibilities and concrete operations are in [Architecture](../architecture.md) and [Current execution paths](roadmap.md).

## Lifetime of facts

Bindings, control flow and operation shapes determined at compile time are written into the published execution words and metadata. Runtime value types, object layouts, property slots, reference counts and aliases are verified at the point of use. `FrameSlots` and heap borrows confine these dynamic facts to one short operation; callbacks, suspension, observable cleanup and frame transitions end that access scope.

## Ownership

Frame slots, heap edges or owned pending state root values. A move hands an existing owner to the destination; a copy creates an additional owner; a borrow is consumed before the protecting frame or heap access ends. Replacing a target follows the release requirement of the replaced value; `Put`, `Set` and initialization each preserve their semantics for operand depth and owner count.

## Progress and positions

Admission precedes commit. An operation that requests a retry keeps its source, destination, depth and owner responsibilities; a selected property operation continues with its result or owned pending state. `fault_pc`, normal fallthrough and resume positions each follow the ordering of their operation. Getters, Proxy, conversions and host callbacks run through the semantic drivers, and the existing algorithms preserve effect order.

## Measurement

A performance judgment records whole-program time, fixed workloads, retired instructions, compilation, code size and lifetime together. Plain builds provide the formal time samples; profiling builds provide event classification. Source, binary, workload, machine, sample order and output validation together identify one measurement. The procedure is in [Measurement method](measurement.md).
