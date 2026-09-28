# Captured-cell and shape storage

This workstream began at frozen PR #53 source
`10262309c580c43ba984b4f371acdc081147b4f0` and was then rebased onto
the current #53 head `38e9eb86e2c2db8fb2e607c9f3ff611407941b4b`.
It delivers two complete implementations in one stacked PR: captured cells
first (`4eedc2aa`), then shapes (`4d79a7e3`). The original frozen-checkpoint
measurements use cell-only `161ae860` and combined `ca70c00e`; the current
head is measured separately. Identities, results and validation are in the
[receipt](receipts/typed-arenas-2026-09-28/README.md).

## Contract

Every production `VarRefData` lives in a compact captured-cell arena, and every
`Shape` lives in a compact shape arena. Their public typed IDs keep full
generation and liveness validation. A cell remains one shared mutable binding
for its active frame and closures, including after frame teardown. The two
replacement operations retain their distinct contracts: ordinary replacement
retains the incoming edges and releases the displaced edges; owned replacement
transfers the incoming owner and returns the displaced owner.

Shape storage preserves copy-on-write for shared layouts, unique-owner in-place
mutation, layout revision invalidation, dictionary order, prototype ownership,
property-key atom ownership, and full-identity weak interner cleanup. Changing
physical storage does not introduce open/closed upvalues, a new value format,
new opcodes, or a new cycle collector.

## Physical layout and lifecycle

The shared `ArenaSlot` holds Object, Context and FunctionBytecode; its payload
enum no longer has `VarRef` or `Shape` variants. Cells and shapes use separate
`AuxiliaryArena<T>` instances with their own slot vectors, free lists and
generations. The slots store a count and inline payload when live, and have
Vacant, Initializing, Live, ZeroQueued and Retired states. They do not carry
object weak-list links or object-only zombie state. A saturated generation
retires its slot; an immortal count never decrements.

The arena handles reservation, publication, typed access and reclamation. The
heap handles outgoing edges, atom cleanup and zero-queue draining. The zero
queue carries full `RawId`s, so a release cascade can cross arenas. Collection
keeps the existing trial-deletion algorithm but uses per-arena trial and
reachability arrays with a `RawId` worklist. Only Object, Context and
FunctionBytecode remain active finalization anchors; cell and shape edges are
still traced. Equal numeric indices in different arenas never imply equal
identity.

The follow-up arena interface associates `VarRefData` with `VarRefId` and
`Shape` with `ShapeId`. Reservation returns the associated ID, and publication,
lookup, release and reclamation accept that ID. The diagnostic kind comes from
the payload type. Heterogeneous heap and collector paths match `RawId` before
entering either arena; generation and liveness checks remain in the arena.

## Implementation sequence and evidence

1. The frozen #53 build establishes common-slot capacity and relevant node
   populations. The cell-only checkpoint covers allocation, access, sharing,
   both mutations, cycle tracing, queued destruction and reuse. It removes the
   resident cell enum variant.
2. The shape checkpoint reuses the compact slot mechanics while covering
   interning, revisions, prototype and atom edges, mutation, collection and
   full-generation reuse. It removes the resident shape enum variant.
3. The receipt compares frozen #53, cell-only and combined builds with the same
   toolchain and configuration. Arena capacity, free lists and collection
   scratch are the main memory evidence; execution timings and allocation
   traces are separate costs. Shape-owned vectors/maps are distinguished from
   the inline slot savings. The work is accepted on measured integrated
   behavior and lifecycle correctness, not a fixed desktop throughput gate.

The source tests exercise typed slot occupancy, index collisions, abort and
reuse, saturation and immortal counts, cell replacement ownership, cycles,
shape interner cleanup and layout behavior. Language and full project tests
remain the integration proof. A smaller `size_of` alone is not a memory result:
the receipt compares reserved capacity at live, drop, collection and reuse
checkpoints.
