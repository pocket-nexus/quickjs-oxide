# S17 append transitions and storage boundaries

Append transitions are weak runtime-local edges from a live `ShapeId` and
`ShapeEntry` to another live `ShapeId`. They store no property values or owning
runtime roots. A reverse index removes incoming edges when a target is reclaimed;
outgoing edges are removed with their source. Shape collection, unique append,
dictionary conversion and descriptor reconfiguration unlink affected edges.
The reverse index bounds cleanup by the affected degree rather than scanning
all runtime shapes. Shared append misses use the existing fingerprint interner;
hits skip copying and hashing the complete shape entries.

Objects with one or more entries may use exclusive in-place append. Their
fingerprint and transitions are removed before mutation. Shared layouts use
append transitions. Eligible deletions enter dictionary mode, detach shared
metadata once, and remove the slot with dictionary insertion-order links.
Other exotic layouts retain their descriptor-specific fallback. Lowering the
unique threshold does not permit shared metadata mutation.

Small shape lookup scans at most eight atoms and has no allocated lookup map.
Larger shapes build an Fx table. Atom hashing emits packed index/generation in
one write; equality still includes runtime domain, so cross-domain handles cannot
alias. Collection bucket keys already contain a keyed hash and use identity
hashing; arbitrary JavaScript keys retain a separately seeded hasher. Each live
record stores its hash for deletion, and a bounded weak string cache includes
short strings without keeping their payloads alive.

Dense slice runs only after coercions and species selection. It copies a fully
present own dense range into an extensible empty dense Array whose writable
length already equals the requested count. Sparse, accessor, proxy and unsuitable
species results keep the ordinary operation path. The heap retains all copied
edges transactionally before publishing the buffer; the runtime retains and
rolls back atom ownership around that publication.

Pinned property atoms are created once per runtime. Production literal call
sites select a generated table entry directly. Integer iterator keys use the
immediate-index path. Pinned handles retain runtime/domain validation while
avoiding repeated string creation and interning.
