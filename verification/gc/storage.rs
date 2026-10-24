//! Storage and root protocol lemmas. These state their abstraction boundary:
//! interval arithmetic, rather than an unproved claim about the entire allocator.
use vstd::prelude::*;
verus! {

pub struct Block { pub start: int, pub end: int }
impl Block {
    pub open spec fn valid(self) -> bool { 0 <= self.start <= self.end }
    pub open spec fn contains(self, x: int) -> bool { self.start <= x < self.end }
    pub open spec fn disjoint(self, b: Block) -> bool {
        forall|x: int| !(self.contains(x) && b.contains(x))
    }
}

pub proof fn split(b: Block, cut: int) -> (parts: (Block, Block))
    requires b.valid(), b.start <= cut <= b.end,
    ensures parts.0.valid(), parts.1.valid(), parts.0.disjoint(parts.1),
        forall|x: int| b.contains(x) <==> parts.0.contains(x) || parts.1.contains(x),
{
    (Block { start: b.start, end: cut }, Block { start: cut, end: b.end })
}

pub proof fn coalesce(a: Block, b: Block) -> (combined: Block)
    requires a.valid(), b.valid(), a.end == b.start,
    ensures combined.valid(),
        forall|x: int| combined.contains(x) <==> a.contains(x) || b.contains(x),
{
    Block { start: a.start, end: b.end }
}

pub proof fn allocate_from_free(owned: Set<int>, free: Set<int>, allocation: Set<int>)
    requires owned.disjoint(free), allocation.subset_of(free),
    ensures
        owned.union(allocation).disjoint(free.difference(allocation)),
        owned.union(allocation).union(free.difference(allocation)) == owned.union(free),
        owned.disjoint(allocation),
{
    assert(owned.union(allocation).union(free.difference(allocation)) =~= owned.union(free));
}

pub proof fn release_to_free(owned: Set<int>, free: Set<int>, released: Set<int>)
    requires owned.disjoint(free), released.subset_of(owned),
    ensures
        owned.difference(released).disjoint(free.union(released)),
        owned.difference(released).union(free.union(released)) == owned.union(free),
{
    assert(owned.difference(released).union(free.union(released)) =~= owned.union(free));
}

// A quarantine is part of the partition, but cannot be allocated yet.
pub proof fn release_quarantine(owned: Set<int>, free: Set<int>, quarantine: Set<int>)
    requires owned.disjoint(free), owned.disjoint(quarantine), free.disjoint(quarantine),
    ensures owned.disjoint(free.union(quarantine)),
{}

// Root marks are positions. Restoring a mark must not remove a retained prefix.
pub proof fn restore_root_mark(roots: Seq<int>, mark: int, keep: int)
    requires 0 <= keep <= mark <= roots.len(),
    ensures roots.take(mark).take(keep) == roots.take(keep),
{
    assert(roots.take(mark).take(keep) =~= roots.take(keep));
}

pub fn main() {}
}
