//! Sequence refinement of the graph worklist, including duplicate entries.
//! This is a representation theorem, not a proof of Heap::drain's field tracing.
use vstd::prelude::*;
mod model;
use model::{Graph, Work};

verus! {

pub struct Queued {
    pub state: Work,
    pub queue: Seq<int>,
}

impl Queued {
    pub open spec fn valid(self, g: Graph) -> bool {
        self.state.valid(g) && self.state.pending == self.queue.to_set()
    }
}

// Already marked items may appear any number of times, or be removed. They
// carry no unmet reachability obligation, but do consume real queue steps.
pub proof fn replace_marked_pending(g: Graph, w: Work, pending: Set<int>) -> (next: Work)
    requires w.valid(g),
        w.pending.subset_of(pending.union(w.marked)),
        pending.subset_of(w.pending.union(w.marked)),
    ensures next.valid(g), next.pending == pending,
        next.marked == w.marked, next.expanded == w.expanded,
        next.scanned == w.scanned, next.waiting == w.waiting,
{
    Work { pending, ..w }
}

pub open spec fn emitted(g: Graph, w: Work, n: int) -> Set<int> {
    g.nodes.filter(|v: int| g.strong.contains((n, v)))
        .union(g.nodes.filter(|v: int| exists|m: int| w.waiting.contains((m, n, v))))
}

pub proof fn pop_duplicate(g: Graph, q: Queued) -> (next: Queued)
    requires q.valid(g), q.queue.len() > 0, q.state.marked.contains(q.queue.last()),
    ensures next.valid(g), next.queue == q.queue.drop_last(),
        next.state.marked == q.state.marked,
        next.state.scanned == q.state.scanned, next.state.waiting == q.state.waiting,
        q.state.pending.subset_of(next.state.pending.union(next.state.marked)),
        next.queue.len() < q.queue.len(),
{
    let rest = q.queue.drop_last();
    assert(q.queue == rest.push(q.queue.last()));
    let state = replace_marked_pending(g, q.state, rest.to_set());
    Queued { state, queue: rest }
}

// The emitted sequence can use any order and any positive multiplicities.
// The production trace adapter must prove that its edges satisfy this premise.
pub proof fn pop_fresh(g: Graph, q: Queued, edges: Seq<int>) -> (next: Queued)
    requires g.valid(), q.valid(g), q.queue.len() > 0,
        !q.state.marked.contains(q.queue.last()),
        edges.to_set() == emitted(g, q.state, q.queue.last()),
    ensures next.valid(g), next.queue == q.queue.drop_last() + edges,
        next.state.marked == q.state.marked.insert(q.queue.last()),
        next.state.scanned == q.state.scanned,
        q.state.pending.subset_of(next.state.pending.union(next.state.marked)),
        g.nodes.difference(next.state.marked).len() < g.nodes.difference(q.state.marked).len(),
{
    let n = q.queue.last();
    let rest = q.queue.drop_last();
    assert(q.queue == rest.push(n));
    let state = model::discover(g, q.state, n);
    let queue = rest + edges;
    vstd::seq_lib::seq_to_set_distributes_over_add(rest, edges);
    let state = replace_marked_pending(g, state, queue.to_set());
    model::least_fixed_point(g);
    g.nodes.lemma_set_insert_diff_decreases(q.state.marked, n);
    Queued { state, queue }
}

// Unlike the set model, termination measures actual queue length. A duplicate
// pop need not remove the node from the queue's set projection.
pub proof fn drain(g: Graph, q: Queued) -> (done: Queued)
    requires g.valid(), q.valid(g),
    ensures done.valid(g), done.queue.len() == 0, done.state.pending.is_empty(),
        q.state.marked.subset_of(done.state.marked),
        q.state.pending.subset_of(done.state.marked),
        done.state.scanned == q.state.scanned,
    decreases g.nodes.difference(q.state.marked).len(), q.queue.len(),
{
    if q.queue.len() == 0 {
        q
    } else if q.state.marked.contains(q.queue.last()) {
        let next = pop_duplicate(g, q);
        drain(g, next)
    } else {
        let outgoing = emitted(g, q.state, q.queue.last());
        outgoing.lemma_to_seq_to_set_id();
        let next = pop_fresh(g, q, outgoing.to_seq());
        drain(g, next)
    }
}

// A duplicate-only queue is legal and drains without marking progress. This
// witnesses why the phase driver's drain precondition must require freshness.
pub proof fn duplicate_only_example()
{
    let g = Graph { nodes: set![0int], roots: set![0int],
        strong: Set::empty(), ephemerons: Set::empty() };
    model::least_fixed_point(g);
    assert(g.reachable() =~= set![0int]);
    let queue = seq![0int, 0int, 0int];
    assert(queue.to_set() =~= set![0int]);
    let q = Queued { queue, state: Work {
        marked: set![0int], expanded: set![0int], pending: set![0int],
        scanned: Set::empty(), waiting: Set::empty(),
    }};
    let popped = pop_duplicate(g, q);
    assert(popped.queue =~= seq![0int, 0int]);
    assert(popped.queue.to_set() =~= set![0int]);
    assert(popped.state.pending == q.state.pending);
    assert(popped.queue.len() == 2);
    let done = drain(g, q);
    assert(done.state.marked == q.state.marked);
}

pub fn main() {}
}
