//! Mathematical contracts for the non-moving collector.
//! Run with scripts/verification/gc.py; this is not a Cargo crate.
use vstd::prelude::*;

verus! {

pub struct Graph {
    pub nodes: Set<int>,
    pub roots: Set<int>,
    pub strong: Set<(int, int)>,
    pub ephemerons: Set<(int, int, int)>,
}

impl Graph {
    pub open spec fn valid(self) -> bool {
        self.roots.subset_of(self.nodes)
        && (forall|a: int, b: int| self.strong.contains((a, b)) ==>
            self.nodes.contains(a) && self.nodes.contains(b))
        && (forall|m: int, k: int, v: int| self.ephemerons.contains((m, k, v)) ==>
            self.nodes.contains(m) && self.nodes.contains(k) && self.nodes.contains(v))
    }

    pub open spec fn closed(self, s: Set<int>) -> bool {
        self.roots.subset_of(s)
        && (forall|a: int, b: int| self.strong.contains((a, b)) && s.contains(a)
            ==> s.contains(b))
        && (forall|m: int, k: int, v: int| self.ephemerons.contains((m, k, v))
            && s.contains(m) && s.contains(k) ==> s.contains(v))
    }

    // Intersection of every closed set: no implementation traversal is used.
    pub open spec fn reachable(self) -> Set<int> {
        self.nodes.filter(|n: int| forall|s: Set<int>| self.closed(s) ==> s.contains(n))
    }
}

pub proof fn least_fixed_point(g: Graph)
    requires g.valid(),
    ensures
        g.closed(g.reachable()),
        g.reachable().subset_of(g.nodes),
        forall|s: Set<int>| g.closed(s) ==> g.reachable().subset_of(s),
{
    assert(g.closed(g.nodes));
    assert forall|n: int| g.roots.contains(n) implies g.reachable().contains(n) by {
        assert forall|s: Set<int>| g.closed(s) implies s.contains(n) by {}
    }
    assert forall|a: int, b: int| g.strong.contains((a, b)) && g.reachable().contains(a)
        implies g.reachable().contains(b) by {
        assert forall|s: Set<int>| g.closed(s) implies s.contains(b) by {}
    }
    assert forall|m: int, k: int, v: int| g.ephemerons.contains((m, k, v))
        && g.reachable().contains(m) && g.reachable().contains(k)
        implies g.reachable().contains(v) by {
        assert forall|s: Set<int>| g.closed(s) implies s.contains(v) by {}
    }
}

// Protocol state at stable boundaries, after a node expansion or key activation.
// Waiting retains the source map as ghost provenance even though the executable
// table stores only key -> values. Sets intentionally ignore duplicate queue items.
pub struct Work {
    pub marked: Set<int>,
    pub pending: Set<int>,
    pub expanded: Set<int>,
    pub scanned: Set<int>,
    pub waiting: Set<(int, int, int)>,
}

impl Work {
    pub open spec fn valid(self, g: Graph) -> bool {
        self.marked.subset_of(g.reachable())
        && self.pending.subset_of(g.reachable())
        && self.expanded == self.marked
        && self.scanned.subset_of(self.marked)
        && g.roots.subset_of(self.marked.union(self.pending))
        && self.marked.subset_of(self.expanded.union(self.pending))
        && (forall|a: int, b: int| g.strong.contains((a, b)) && self.expanded.contains(a)
            ==> self.marked.contains(b) || self.pending.contains(b))
        && (forall|m: int, k: int, v: int| g.ephemerons.contains((m, k, v))
            && self.scanned.contains(m) ==> self.marked.contains(v)
                || self.pending.contains(v) || self.waiting.contains((m, k, v)))
        && (forall|m: int, k: int, v: int| self.waiting.contains((m, k, v)) ==>
            g.ephemerons.contains((m, k, v)) && self.scanned.contains(m)
                && !self.marked.contains(k))
    }
}

pub proof fn initial(g: Graph) -> (w: Work)
    requires g.valid(),
    ensures w.valid(g), w.marked.is_empty(), w.waiting.is_empty(),
{
    least_fixed_point(g);
    Work { marked: Set::empty(), pending: g.roots, expanded: Set::empty(),
        scanned: Set::empty(), waiting: Set::empty() }
}

// One semantic discovery event: mark, expand strong edges, activate waiters.
// This combines the consecutive operations in drain into a stable transition.
pub proof fn discover(g: Graph, w: Work, n: int) -> (next: Work)
    requires g.valid(), w.valid(g), w.pending.contains(n), !w.marked.contains(n),
    ensures next.valid(g), next.marked == w.marked.insert(n),
        next.expanded == w.expanded.insert(n), next.scanned == w.scanned,
        next.pending == w.pending.remove(n)
            .union(g.nodes.filter(|v: int| g.strong.contains((n, v))))
            .union(g.nodes.filter(|v: int| exists|m: int| w.waiting.contains((m, n, v)))),
        next.waiting == w.waiting.filter(|e: (int, int, int)| e.1 != n),
{
    least_fixed_point(g);
    let children = g.nodes.filter(|v: int| g.strong.contains((n, v)));
    let awakened = g.nodes.filter(|v: int| exists|m: int| w.waiting.contains((m, n, v)));
    let waiting = w.waiting.filter(|e: (int, int, int)| e.1 != n);
    assert forall|v: int| awakened.contains(v) implies g.reachable().contains(v) by {
        let m = choose|m: int| w.waiting.contains((m, n, v));
        assert(g.ephemerons.contains((m, n, v)));
    }
    Work {
        marked: w.marked.insert(n),
        pending: w.pending.remove(n).union(children).union(awakened),
        expanded: w.expanded.insert(n), scanned: w.scanned, waiting,
    }
}

pub proof fn discard_duplicate(g: Graph, w: Work, n: int) -> (next: Work)
    requires g.valid(), w.valid(g), w.pending.contains(n), w.marked.contains(n),
    ensures next.valid(g), next.marked == w.marked, next.scanned == w.scanned,
        next.pending == w.pending.remove(n),
{
    Work { pending: w.pending.remove(n), ..w }
}

// Scan a newly discovered map. Its identity is essential to soundness.
pub proof fn scan_map(g: Graph, w: Work, m: int) -> (next: Work)
    requires g.valid(), w.valid(g), w.marked.contains(m),
    ensures next.valid(g), next.marked == w.marked,
        next.scanned == w.scanned.insert(m),
{
    least_fixed_point(g);
    let ready = g.nodes.filter(|v: int| exists|k: int|
        g.ephemerons.contains((m, k, v)) && w.marked.contains(k));
    let deferred = g.ephemerons.filter(|e: (int, int, int)| e.0 == m && !w.marked.contains(e.1));
    assert forall|v: int| ready.contains(v) implies g.reachable().contains(v) by {
        let k = choose|k: int| g.ephemerons.contains((m, k, v)) && w.marked.contains(k);
        assert(g.reachable().contains(k));
    }
    Work { marked: w.marked, pending: w.pending.union(ready), expanded: w.expanded,
        scanned: w.scanned.insert(m), waiting: w.waiting.union(deferred) }
}

pub proof fn finish(g: Graph, w: Work)
    requires g.valid(), w.valid(g), w.pending.is_empty(),
        forall|m: int, k: int, v: int| g.ephemerons.contains((m, k, v))
            && w.marked.contains(m) ==> w.scanned.contains(m),
    ensures w.marked == g.reachable(),
{
    least_fixed_point(g);
    assert(g.closed(w.marked));
    assert(w.marked =~= g.reachable());
}

// Total abstract execution. No fixed heap size or iteration bound. A fresh
// discovery decreases unmarked nodes; scanning decreases unscanned nodes;
// discarding a duplicate decreases pending work. The production implementation
// additionally uses sequence multiplicities, refined separately in worklist.rs;
// linking those sequence transitions to the production tracer remains open.
pub proof fn run(g: Graph, w: Work) -> (done: Work)
    requires g.valid(), w.valid(g),
    ensures done.valid(g), done.marked == g.reachable(), done.pending.is_empty(),
    decreases g.nodes.difference(w.marked).len(),
        g.nodes.difference(w.scanned).len(), w.pending.len(),
{
    least_fixed_point(g);
    if !w.pending.is_empty() {
        let n = w.pending.choose();
        if w.marked.contains(n) {
            let next = discard_duplicate(g, w, n);
            run(g, next)
        } else {
            let next = discover(g, w, n);
            g.nodes.lemma_set_insert_diff_decreases(w.marked, n);
            run(g, next)
        }
    } else if exists|m: int, k: int, v: int| g.ephemerons.contains((m, k, v))
        && w.marked.contains(m) && !w.scanned.contains(m) {
        let e = choose|e: (int, int, int)| g.ephemerons.contains(e)
            && w.marked.contains(e.0) && !w.scanned.contains(e.0);
        let next = scan_map(g, w, e.0);
        g.nodes.lemma_set_insert_diff_decreases(w.scanned, e.0);
        run(g, next)
    } else {
        finish(g, w);
        w
    }
}

// Sweep's set-level contract; byte layout and bookkeeping are separate obligations.
pub proof fn sweep(g: Graph, marked: Set<int>, old_live: Set<int>) -> (live: Set<int>)
    requires g.valid(), marked == g.reachable(), g.nodes == old_live,
    ensures live == g.reachable(),
        old_live.difference(live).disjoint(g.reachable()),
{
    least_fixed_point(g);
    let live = old_live.intersect(marked);
    assert(live =~= marked);
    live
}

// Rooting is an explicit caller obligation, not implied by collector correctness.
pub proof fn safepoint_preserves_needed(g: Graph, needed: Set<int>, after: Set<int>)
    requires g.valid(), needed.subset_of(g.reachable()), after == g.reachable(),
    ensures needed.subset_of(after),
{}

// Finalization jobs add roots that were already kept by strong registry edges.
pub proof fn add_kept_roots(g: Graph, added: Set<int>)
    requires g.valid(), added.subset_of(g.reachable()),
    ensures (Graph { roots: g.roots.union(added), ..g }).reachable() == g.reachable(),
{
    least_fixed_point(g);
    let next = Graph { roots: g.roots.union(added), ..g };
    least_fixed_point(next);
    assert(next.closed(g.reachable()));
    assert(g.roots.subset_of(next.reachable()));
    assert forall|a: int, b: int| g.strong.contains((a, b)) && next.reachable().contains(a)
        implies next.reachable().contains(b) by {
        assert(next.strong.contains((a, b)));
    }
    assert forall|m: int, k: int, v: int| g.ephemerons.contains((m, k, v))
        && next.reachable().contains(m) && next.reachable().contains(k)
        implies next.reachable().contains(v) by {
        assert(next.ephemerons.contains((m, k, v)));
    }
    assert(g.closed(next.reachable()));
    assert(next.reachable() =~= g.reachable());
}

// Caches surviving pruning name only retained records, before indices are reused.
pub proof fn prune_before_reuse(live: Set<int>, named: Set<int>, reused: Set<int>)
    requires named.subset_of(live), reused.disjoint(live),
    ensures named.disjoint(reused),
{}

// A closed witness rules out self-supporting weak cycles without exploring graphs.
pub proof fn excluded_by_closed_witness(g: Graph, witness: Set<int>, node: int)
    requires g.valid(), g.closed(witness), !witness.contains(node),
    ensures !g.reachable().contains(node),
{
    least_fixed_point(g);
}

// Concrete sanity theorems also demonstrate that the graph preconditions are
// satisfiable. They are not substitutes for the universally quantified proofs.
pub proof fn weak_cycle_example()
{
    let g = Graph { nodes: set![0int, 1int, 2int], roots: set![0int],
        strong: set![(2int, 1int)], ephemerons: set![(0int, 1int, 2int)] };
    assert(g.valid());
    let w = initial(g);
    let done = run(g, w);
    let witness = set![0int];
    assert(g.closed(witness));
    excluded_by_closed_witness(g, witness, 1);
    excluded_by_closed_witness(g, witness, 2);
    assert(done.marked == witness);
}

pub proof fn dead_map_example()
{
    let g = Graph { nodes: set![0int, 1int, 2int], roots: set![1int],
        strong: Set::empty(), ephemerons: set![(0int, 1int, 2int)] };
    let start = initial(g);
    let done = run(g, start);
    let witness = set![1int];
    assert(g.closed(witness));
    excluded_by_closed_witness(g, witness, 0);
    excluded_by_closed_witness(g, witness, 2);
    assert(done.marked == witness);
}

pub proof fn newly_discovered_map_example()
{
    let g = Graph { nodes: set![0int, 1int, 2int, 3int, 4int, 5int], roots: set![0int, 1int],
        strong: set![(2int, 3int), (2int, 4int)],
        ephemerons: set![(0int, 1int, 2int), (3int, 4int, 5int)] };
    let start = initial(g);
    let done = run(g, start);
    least_fixed_point(g);
    assert(g.reachable().contains(0));
    assert(g.reachable().contains(1));
    assert(g.ephemerons.contains((0, 1, 2)));
    assert(g.reachable().contains(2));
    assert(g.strong.contains((2, 3)));
    assert(g.reachable().contains(3));
    assert(g.strong.contains((2, 4)));
    assert(g.reachable().contains(4));
    assert(g.ephemerons.contains((3, 4, 5)));
    assert(g.reachable().contains(5));
    assert(done.marked == g.nodes);
}

pub proof fn next_collection_ignores_previous_work(g: Graph, next: Graph, previous: Work) -> (done: Work)
    requires g.valid(), previous.valid(g), next.valid(),
    ensures done.marked == next.reachable(),
{
    let start = initial(next);
    run(next, start)
}

pub fn main() {}
}
